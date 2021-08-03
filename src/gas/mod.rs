//! This module is used to instrument a Wasm module with gas metering code.
//!
//! The primary public interface is the `inject_gas_counter` function which transforms a given
//! module into one that charges gas for code to be executed. See function documentation for usage
//! and details.

#[cfg(test)]
mod validation;
pub mod global_based_counter;

use std::mem;
use std::vec::Vec;

use parity_wasm::{elements, builder};
use rules;

pub use self::global_based_counter::update_call_index;
use self::global_based_counter::{MeteredBlock, determine_metered_blocks, inject_grow_counter};

fn add_grow_counter(module: elements::Module, rules: &rules::Set, gas_func: u32) -> elements::Module {
	use parity_wasm::elements::Instruction::*;

	let mut b = builder::from_module(module);
	b.push_function(
		builder::function()
			.signature().params().i32().build().with_return_type(Some(elements::ValueType::I32)).build()
			.body()
			.with_instructions(elements::Instructions::new(vec![
				GetLocal(0),
				GetLocal(0),
				I32Const(rules.grow_cost() as i32),
				I32Mul,
				// todo: there should be strong guarantee that it does not return anything on stack?
				Call(gas_func),
				GrowMemory(0),
				End,
			]))
			.build()
			.build()
	);

	b.build()
}

pub fn inject_counter(
	instructions: &mut elements::Instructions,
	rules: &rules::Set,
	gas_func: u32,
) -> Result<(), ()> {
	let blocks = determine_metered_blocks(instructions, rules)?;
	insert_metering_calls(instructions, blocks, gas_func)
}

// Then insert metering calls into a sequence of instructions given the block locations and costs.
fn insert_metering_calls(
	instructions: &mut elements::Instructions,
	blocks: Vec<MeteredBlock>,
	gas_func: u32,
)
	-> Result<(), ()>
{
	use parity_wasm::elements::Instruction::*;

	// To do this in linear time, construct a new vector of instructions, copying over old
	// instructions one by one and injecting new ones as required.
	let new_instrs_len = instructions.elements().len() + 2 * blocks.len();
	let original_instrs = mem::replace(
		instructions.elements_mut(), Vec::with_capacity(new_instrs_len)
	);
	let new_instrs = instructions.elements_mut();

	let mut block_iter = blocks.into_iter().peekable();
	for (original_pos, instr) in original_instrs.into_iter().enumerate() {
		// If there the next block starts at this position, inject metering instructions.
		let used_block = if let Some(ref block) = block_iter.peek() {
			if block.start_pos == original_pos {
				new_instrs.push(I32Const(block.cost as i32));
				new_instrs.push(Call(gas_func));
				true
			} else { false }
		} else { false };

		if used_block {
			block_iter.next();
		}

		// Copy over the original instruction.
		new_instrs.push(instr);
	}

	if block_iter.next().is_some() {
		return Err(());
	}

	Ok(())
}

/// Transforms a given module into one that charges gas for code to be executed by proxy of an
/// imported gas metering function.
///
/// The output module imports a function "gas" from the module "env" with type signature
/// [i32] -> []. The argument is the amount of gas required to continue execution. The external
/// function is meant to keep track of the total amount of gas used and trap or otherwise halt
/// execution of the runtime if the gas usage exceeds some allowed limit.
///
/// The body of each function is divided into metered blocks, and the calls to charge gas are
/// inserted at the beginning of every such block of code. A metered block is defined so that,
/// unless there is a trap, either all of the instructions are executed or none are. These are
/// similar to basic blocks in a control flow graph, except that in some cases multiple basic
/// blocks can be merged into a single metered block. This is the case if any path through the
/// control flow graph containing one basic block also contains another.
///
/// Charging gas is at the beginning of each metered block ensures that 1) all instructions
/// executed are already paid for, 2) instructions that will not be executed are not charged for
/// unless execution traps, and 3) the number of calls to "gas" is minimized. The corollary is that
/// modules instrumented with this metering code may charge gas for instructions not executed in
/// the event of a trap.
///
/// Additionally, each `memory.grow` instruction found in the module is instrumented to first make
/// a call to charge gas for the additional pages requested. This cannot be done as part of the
/// block level gas charges as the gas cost is not static and depends on the stack argument to
/// `memory.grow`.
///
/// The above transformations are performed for every function body defined in the module. This
/// function also rewrites all function indices references by code, table elements, etc., since
/// the addition of an imported functions changes the indices of module-defined functions.
///
/// This routine runs in time linear in the size of the input module.
///
/// The function fails if the module contains any operation forbidden by gas rule set, returning
/// the original module as an Err.
pub fn inject_gas_counter(module: elements::Module, rules: &rules::Set)
						  -> Result<elements::Module, elements::Module>
{
	// Injecting gas counting external
	let mut mbuilder = builder::from_module(module);
	let import_sig = mbuilder.push_signature(
		builder::signature()
			.param().i32()
			.build_sig()
	);

	mbuilder.push_import(
		builder::import()
			.module("env")
			.field("gas")
			.external().func(import_sig)
			.build()
	);

	// back to plain module
	let mut module = mbuilder.build();

	// calculate actual function index of the imported definition
	//    (subtract all imports that are NOT functions)

	let gas_func = module.import_count(elements::ImportCountType::Function) as u32 - 1;
	let total_func = module.functions_space() as u32;
	let mut need_grow_counter = false;
	let mut error = false;

	// Updating calling addresses (all calls to function index >= `gas_func` should be incremented)
	for section in module.sections_mut() {
		match section {
			&mut elements::Section::Code(ref mut code_section) => {
				for ref mut func_body in code_section.bodies_mut() {
					update_call_index(func_body.code_mut(), gas_func);
					if let Err(_) = inject_counter(func_body.code_mut(), rules, gas_func) {
						error = true;
						break;
					}
					if rules.grow_cost() > 0 {
						if inject_grow_counter(func_body.code_mut(), total_func) > 0 {
							need_grow_counter = true;
						}
					}
				}
			},
			&mut elements::Section::Export(ref mut export_section) => {
				for ref mut export in export_section.entries_mut() {
					if let &mut elements::Internal::Function(ref mut func_index) = export.internal_mut() {
						if *func_index >= gas_func { *func_index += 1}
					}
				}
			},
			&mut elements::Section::Element(ref mut elements_section) => {
				// Note that we do not need to check the element type referenced because in the
				// WebAssembly 1.0 spec, the only allowed element type is funcref.
				for ref mut segment in elements_section.entries_mut() {
					// update all indirect call addresses initial values
					for func_index in segment.members_mut() {
						if *func_index >= gas_func { *func_index += 1}
					}
				}
			},
			&mut elements::Section::Start(ref mut start_idx) => {
				if *start_idx >= gas_func { *start_idx += 1}
			},
			_ => { }
		}
	}

	if error { return Err(module); }

	if need_grow_counter { Ok(add_grow_counter(module, rules, gas_func)) } else { Ok(module) }
}

#[cfg(test)]
mod tests {

	extern crate wabt;

	use parity_wasm::{serialize, builder, elements};
	use parity_wasm::elements::Instruction::*;
	use super::*;
	use rules;

	pub fn get_function_body(module: &elements::Module, index: usize)
							 -> Option<&[elements::Instruction]>
	{
		module.code_section()
			.and_then(|code_section| code_section.bodies().get(index))
			.map(|func_body| func_body.code().elements())
	}

	#[test]
	fn simple_grow() {
		let module = builder::module()
			.global()
			.value_type().i32()
			.build()
			.function()
			.signature().param().i32().build()
			.body()
			.with_instructions(elements::Instructions::new(
				vec![
					GetGlobal(0),
					GrowMemory(0),
					End
				]
			))
			.build()
			.build()
			.build();

		let injected_module = inject_gas_counter(module, &rules::Set::default().with_grow_cost(10000)).unwrap();

		assert_eq!(
			get_function_body(&injected_module, 0).unwrap(),
			&vec![
				I32Const(2),
				Call(0),
				GetGlobal(0),
				Call(2),
				End
			][..]
		);
		assert_eq!(
			get_function_body(&injected_module, 1).unwrap(),
			&vec![
				GetLocal(0),
				GetLocal(0),
				I32Const(10000),
				I32Mul,
				Call(0),
				GrowMemory(0),
				End,
			][..]
		);

		let binary = serialize(injected_module).expect("serialization failed");
		self::wabt::wasm2wat(&binary).unwrap();
	}

	#[test]
	fn grow_no_gas_no_track() {
		let module = builder::module()
			.global()
			.value_type().i32()
			.build()
			.function()
			.signature().param().i32().build()
			.body()
			.with_instructions(elements::Instructions::new(
				vec![
					GetGlobal(0),
					GrowMemory(0),
					End
				]
			))
			.build()
			.build()
			.build();

		let injected_module = inject_gas_counter(module, &rules::Set::default()).unwrap();

		assert_eq!(
			get_function_body(&injected_module, 0).unwrap(),
			&vec![
				I32Const(2),
				Call(0),
				GetGlobal(0),
				GrowMemory(0),
				End
			][..]
		);

		assert_eq!(injected_module.functions_space(), 2);

		let binary = serialize(injected_module).expect("serialization failed");
		self::wabt::wasm2wat(&binary).unwrap();
	}

	#[test]
	fn call_index() {
		let module = builder::module()
			.global()
			.value_type().i32()
			.build()
			.function()
			.signature().param().i32().build()
			.body().build()
			.build()
			.function()
			.signature().param().i32().build()
			.body()
			.with_instructions(elements::Instructions::new(
				vec![
					Call(0),
					If(elements::BlockType::NoResult),
					Call(0),
					Call(0),
					Call(0),
					Else,
					Call(0),
					Call(0),
					End,
					Call(0),
					End
				]
			))
			.build()
			.build()
			.build();

		let injected_module = inject_gas_counter(module, &Default::default()).unwrap();

		assert_eq!(
			get_function_body(&injected_module, 1).unwrap(),
			&vec![
				I32Const(3),
				Call(0),
				Call(1),
				If(elements::BlockType::NoResult),
				I32Const(3),
				Call(0),
				Call(1),
				Call(1),
				Call(1),
				Else,
				I32Const(2),
				Call(0),
				Call(1),
				Call(1),
				End,
				Call(1),
				End
			][..]
		);
	}

	#[test]
	fn forbidden() {
		let module = builder::module()
			.global()
			.value_type().i32()
			.build()
			.function()
			.signature().param().i32().build()
			.body()
			.with_instructions(elements::Instructions::new(
				vec![
					F32Const(555555),
					End
				]
			))
			.build()
			.build()
			.build();

		let rules = rules::Set::default().with_forbidden_floats();

		if let Err(_) = inject_gas_counter(module, &rules) { }
		else { panic!("Should be error because of the forbidden operation")}

	}

	fn parse_wat(source: &str) -> elements::Module {
		let module_bytes = wabt::Wat2Wasm::new()
			.validate(false)
			.convert(source)
			.expect("failed to parse module");
		elements::deserialize_buffer(module_bytes.as_ref())
			.expect("failed to parse module")
	}

	macro_rules! test_gas_counter_injection {
		(name = $name:ident; input = $input:expr; expected = $expected:expr) => {
			#[test]
			fn $name() {
				let input_module = parse_wat($input);
				let expected_module = parse_wat($expected);

				let injected_module = inject_gas_counter(input_module, &Default::default())
					.expect("inject_gas_counter call failed");

				let actual_func_body = get_function_body(&injected_module, 0)
					.expect("injected module must have a function body");
				let expected_func_body = get_function_body(&expected_module, 0)
					.expect("post-module must have a function body");

				assert_eq!(actual_func_body, expected_func_body);
			}
		}
	}

	test_gas_counter_injection! {
		name = simple;
		input = r#"
		(module
			(func (result i32)
				(get_global 0)))
		"#;
		expected = r#"
		(module
			(func (result i32)
				(call 0 (i32.const 1))
				(get_global 0)))
		"#
	}

	test_gas_counter_injection! {
		name = nested;
		input = r#"
		(module
			(func (result i32)
				(get_global 0)
				(block
					(get_global 0)
					(get_global 0)
					(get_global 0))
				(get_global 0)))
		"#;
		expected = r#"
		(module
			(func (result i32)
				(call 0 (i32.const 6))
				(get_global 0)
				(block
					(get_global 0)
					(get_global 0)
					(get_global 0))
				(get_global 0)))
		"#
	}

	test_gas_counter_injection! {
		name = ifelse;
		input = r#"
		(module
			(func (result i32)
				(get_global 0)
				(if
					(then
						(get_global 0)
						(get_global 0)
						(get_global 0))
					(else
						(get_global 0)
						(get_global 0)))
				(get_global 0)))
		"#;
		expected = r#"
		(module
			(func (result i32)
				(call 0 (i32.const 3))
				(get_global 0)
				(if
					(then
						(call 0 (i32.const 3))
						(get_global 0)
						(get_global 0)
						(get_global 0))
					(else
						(call 0 (i32.const 2))
						(get_global 0)
						(get_global 0)))
				(get_global 0)))
		"#
	}

	test_gas_counter_injection! {
		name = branch_innermost;
		input = r#"
		(module
			(func (result i32)
				(get_global 0)
				(block
					(get_global 0)
					(drop)
					(br 0)
					(get_global 0)
					(drop))
				(get_global 0)))
		"#;
		expected = r#"
		(module
			(func (result i32)
				(call 0 (i32.const 6))
				(get_global 0)
				(block
					(get_global 0)
					(drop)
					(br 0)
					(call 0 (i32.const 2))
					(get_global 0)
					(drop))
				(get_global 0)))
		"#
	}

	test_gas_counter_injection! {
		name = branch_outer_block;
		input = r#"
		(module
			(func (result i32)
				(get_global 0)
				(block
					(get_global 0)
					(if
						(then
							(get_global 0)
							(get_global 0)
							(drop)
							(br_if 1)))
					(get_global 0)
					(drop))
				(get_global 0)))
		"#;
		expected = r#"
		(module
			(func (result i32)
				(call 0 (i32.const 5))
				(get_global 0)
				(block
					(get_global 0)
					(if
						(then
							(call 0 (i32.const 4))
							(get_global 0)
							(get_global 0)
							(drop)
							(br_if 1)))
					(call 0 (i32.const 2))
					(get_global 0)
					(drop))
				(get_global 0)))
		"#
	}

	test_gas_counter_injection! {
		name = branch_outer_loop;
		input = r#"
		(module
			(func (result i32)
				(get_global 0)
				(loop
					(get_global 0)
					(if
						(then
							(get_global 0)
							(br_if 0))
						(else
							(get_global 0)
							(get_global 0)
							(drop)
							(br_if 1)))
					(get_global 0)
					(drop))
				(get_global 0)))
		"#;
		expected = r#"
		(module
			(func (result i32)
				(call 0 (i32.const 3))
				(get_global 0)
				(loop
					(call 0 (i32.const 4))
					(get_global 0)
					(if
						(then
							(call 0 (i32.const 2))
							(get_global 0)
							(br_if 0))
						(else
							(call 0 (i32.const 4))
							(get_global 0)
							(get_global 0)
							(drop)
							(br_if 1)))
					(get_global 0)
					(drop))
				(get_global 0)))
		"#
	}

	test_gas_counter_injection! {
		name = return_from_func;
		input = r#"
		(module
			(func (result i32)
				(get_global 0)
				(if
					(then
						(return)))
				(get_global 0)))
		"#;
		expected = r#"
		(module
			(func (result i32)
				(call 0 (i32.const 2))
				(get_global 0)
				(if
					(then
						(call 0 (i32.const 1))
						(return)))
				(call 0 (i32.const 1))
				(get_global 0)))
		"#
	}

	test_gas_counter_injection! {
		name = branch_from_if_not_else;
		input = r#"
		(module
			(func (result i32)
				(get_global 0)
				(block
					(get_global 0)
					(if
						(then (br 1))
						(else (br 0)))
					(get_global 0)
					(drop))
				(get_global 0)))
		"#;
		expected = r#"
		(module
			(func (result i32)
				(call 0 (i32.const 5))
				(get_global 0)
				(block
					(get_global 0)
					(if
						(then
							(call 0 (i32.const 1))
							(br 1))
						(else
							(call 0 (i32.const 1))
							(br 0)))
					(call 0 (i32.const 2))
					(get_global 0)
					(drop))
				(get_global 0)))
		"#
	}
}
