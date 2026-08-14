//! Relational Virtual Machine
//!
//! This module demonstrates how a simple Virtual Machine (VM) can be modeled using purely relational
//! algebra. The machine's registers, memory, and program counter are all relations,
//! and computing the next state is performed entirely via relational operators (Join,
//! Extend, Restrict, Union).
//!
//! # Concept
//!
//! - **Registers**: Relation `(reg_id: String, value: Int)`.
//! - **Memory**: Relation `(address: Int, value: Int)`.
//! - **Program**: Relation `(pc: Int, opcode: String, arg1: String, arg2: String, arg3: String)`.
//! - **Head**: Relation `(pc: Int)`.

use relvar_core::{
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue, Tuple},
};

/// A Relational Virtual Machine.
///
/// This demonstrates representing computation via relational algebra operations,
/// turning a CPU's state transitions into relational joins and extends.
///
/// # Examples
///
/// ```
/// use relvar_core::{tuple, Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::vm::RelationalVM;
///
/// // Set up a minimal VM environment:
/// let reg_type = RelationType::new(TupleType::new()
///     .with_attribute("reg_id", ScalarType::String)
///     .with_attribute("value", ScalarType::Int));
/// let mut registers = Relation::new(reg_type);
/// registers.insert(tuple! { reg_id: "R1", value: 5i64 }).unwrap();
///
/// let mem_type = RelationType::new(TupleType::new()
///     .with_attribute("address", ScalarType::Int)
///     .with_attribute("value", ScalarType::Int));
/// let memory = Relation::new(mem_type);
///
/// let prog_type = RelationType::new(TupleType::new()
///     .with_attribute("pc", ScalarType::Int)
///     .with_attribute("opcode", ScalarType::String)
///     .with_attribute("arg1", ScalarType::String)
///     .with_attribute("arg2", ScalarType::String)
///     .with_attribute("arg3", ScalarType::String));
/// let program = Relation::new(prog_type);
///
/// let head_type = RelationType::new(TupleType::new()
///     .with_attribute("pc", ScalarType::Int));
/// let mut head = Relation::new(head_type);
/// head.insert(tuple! { pc: 0i64 }).unwrap();
///
/// let vm = RelationalVM::new(registers, memory, program, head);
/// ```
pub struct RelationalVM {
    /// The registers of the VM. Schema: (reg_id: String, value: Int)
    pub registers: Relation,
    /// The memory of the VM. Schema: (address: Int, value: Int)
    pub memory: Relation,
    /// The program of the VM. Schema: (pc: Int, opcode: String, arg1: String, arg2: String, arg3: String)
    pub program: Relation,
    /// The current program counter. Schema: (pc: Int)
    pub head: Relation,
}

impl RelationalVM {
    /// Creates a new Relational VM.
    ///
    /// This exists to initialize the state relations that hold the VM's current configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::{tuple, Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::vm::RelationalVM;
    ///
    /// let reg_type = RelationType::new(TupleType::new()
    ///     .with_attribute("reg_id", ScalarType::String)
    ///     .with_attribute("value", ScalarType::Int));
    /// let registers = Relation::new(reg_type);
    ///
    /// let mem_type = RelationType::new(TupleType::new()
    ///     .with_attribute("address", ScalarType::Int)
    ///     .with_attribute("value", ScalarType::Int));
    /// let memory = Relation::new(mem_type);
    ///
    /// let prog_type = RelationType::new(TupleType::new()
    ///     .with_attribute("pc", ScalarType::Int)
    ///     .with_attribute("opcode", ScalarType::String)
    ///     .with_attribute("arg1", ScalarType::String)
    ///     .with_attribute("arg2", ScalarType::String)
    ///     .with_attribute("arg3", ScalarType::String));
    /// let program = Relation::new(prog_type);
    ///
    /// let head_type = RelationType::new(TupleType::new()
    ///     .with_attribute("pc", ScalarType::Int));
    /// let head = Relation::new(head_type);
    ///
    /// let vm = RelationalVM::new(registers, memory, program, head);
    /// ```
    pub fn new(registers: Relation, memory: Relation, program: Relation, head: Relation) -> Self {
        Self {
            registers,
            memory,
            program,
            head,
        }
    }

    /// Computes the next step of the VM.
    ///
    /// Evaluates the machine step and yields `true` if it progressed, or `false` if it halted (no matching instruction).
    /// This exists to demonstrate applying relational algebra (joins, restricts, and extends)
    /// to update state.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::{tuple, Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::vm::RelationalVM;
    ///
    /// let reg_type = RelationType::new(TupleType::new()
    ///     .with_attribute("reg_id", ScalarType::String)
    ///     .with_attribute("value", ScalarType::Int));
    /// let mut registers = Relation::new(reg_type);
    /// registers.insert(tuple! { reg_id: "R1", value: 10i64 }).unwrap();
    /// registers.insert(tuple! { reg_id: "R2", value: 20i64 }).unwrap();
    /// registers.insert(tuple! { reg_id: "R3", value: 0i64 }).unwrap();
    ///
    /// let mem_type = RelationType::new(TupleType::new()
    ///     .with_attribute("address", ScalarType::Int)
    ///     .with_attribute("value", ScalarType::Int));
    /// let memory = Relation::new(mem_type);
    ///
    /// let prog_type = RelationType::new(TupleType::new()
    ///     .with_attribute("pc", ScalarType::Int)
    ///     .with_attribute("opcode", ScalarType::String)
    ///     .with_attribute("arg1", ScalarType::String)
    ///     .with_attribute("arg2", ScalarType::String)
    ///     .with_attribute("arg3", ScalarType::String));
    /// let mut program = Relation::new(prog_type);
    /// // ADD instruction: R3 = R1 + R2
    /// program.insert(tuple! { pc: 0i64, opcode: "ADD", arg1: "R3", arg2: "R1", arg3: "R2" }).unwrap();
    ///
    /// let head_type = RelationType::new(TupleType::new()
    ///     .with_attribute("pc", ScalarType::Int));
    /// let mut head = Relation::new(head_type);
    /// head.insert(tuple! { pc: 0i64 }).unwrap();
    ///
    /// let mut vm = RelationalVM::new(registers, memory, program, head);
    ///
    /// // Take one step
    /// let advanced = vm.step().unwrap();
    /// assert!(advanced);
    /// ```
    pub fn step(&mut self) -> Result<bool, DatabaseError> {
        // Fetch current instruction
        let current_inst = self.head.join(&self.program)?;

        if current_inst.cardinality() == 0 {
            return Ok(false);
        }

        // We'll support an "ADD" instruction as a simple example:
        // ADD target_reg, src1_reg, src2_reg

        let add_inst = current_inst
            .clone()
            .restrict(|t: &Tuple| t.get_typed::<String>("opcode").unwrap() == "ADD");

        if add_inst.cardinality() > 0 {
            self.execute_add_instruction(&add_inst)?;
            self.increment_pc()?;
            return Ok(true);
        }

        Ok(false)
    }

    fn execute_add_instruction(&mut self, add_inst: &Relation) -> Result<(), DatabaseError> {
        // Join arg2 with registers to get src1 value
        let src1_join = add_inst
            .rename(&[("arg2", "reg_id")])
            .join(&self.registers)?
            .rename(&[("value", "src1_val")])
            .rename(&[("reg_id", "arg2")]);

        // Join arg3 with registers to get src2 value
        let src2_join = src1_join
            .rename(&[("arg3", "reg_id")])
            .join(&self.registers)?
            .rename(&[("value", "src2_val")])
            .rename(&[("reg_id", "arg3")]);

        // Calculate the new value
        let evaluated = src2_join
            .extend("new_val", ScalarType::Int, |t: &Tuple| {
                let s1 = t.get_typed::<i64>("src1_val").unwrap();
                let s2 = t.get_typed::<i64>("src2_val").unwrap();
                ScalarValue::Int(s1 + s2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Extract the new register update
        let new_reg_update = evaluated
            .project(&["arg1", "new_val"])
            .rename(&[("arg1", "reg_id"), ("new_val", "value")]);

        // Filter out the old register value
        let old_reg = evaluated
            .project(&["arg1"])
            .rename(&[("arg1", "reg_id")])
            .join(&self.registers)?;

        self.registers = self
            .registers
            .difference(&old_reg)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .union(&new_reg_update)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(())
    }

    fn increment_pc(&mut self) -> Result<(), DatabaseError> {
        self.head = self
            .head
            .extend("new_pc", ScalarType::Int, |t: &Tuple| {
                let pc = t.get_typed::<i64>("pc").unwrap();
                ScalarValue::Int(pc + 1)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["new_pc"])
            .rename(&[("new_pc", "pc")]);
        Ok(())
    }

    /// Runs the machine until it halts (returns the number of steps taken).
    ///
    /// This exists to continuously evaluate the relational state machine until an end condition
    /// is met (i.e. `step` returns `false` due to an unhandled or missing program instruction).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::{tuple, Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::vm::RelationalVM;
    ///
    /// let reg_type = RelationType::new(TupleType::new()
    ///     .with_attribute("reg_id", ScalarType::String)
    ///     .with_attribute("value", ScalarType::Int));
    /// let mut registers = Relation::new(reg_type);
    /// registers.insert(tuple! { reg_id: "R1", value: 5i64 }).unwrap();
    /// registers.insert(tuple! { reg_id: "R2", value: 7i64 }).unwrap();
    /// registers.insert(tuple! { reg_id: "R3", value: 0i64 }).unwrap();
    ///
    /// let mem_type = RelationType::new(TupleType::new()
    ///     .with_attribute("address", ScalarType::Int)
    ///     .with_attribute("value", ScalarType::Int));
    /// let memory = Relation::new(mem_type);
    ///
    /// let prog_type = RelationType::new(TupleType::new()
    ///     .with_attribute("pc", ScalarType::Int)
    ///     .with_attribute("opcode", ScalarType::String)
    ///     .with_attribute("arg1", ScalarType::String)
    ///     .with_attribute("arg2", ScalarType::String)
    ///     .with_attribute("arg3", ScalarType::String));
    /// let mut program = Relation::new(prog_type);
    /// // ADD instruction: R3 = R1 + R2
    /// program.insert(tuple! { pc: 0i64, opcode: "ADD", arg1: "R3", arg2: "R1", arg3: "R2" }).unwrap();
    ///
    /// let head_type = RelationType::new(TupleType::new()
    ///     .with_attribute("pc", ScalarType::Int));
    /// let mut head = Relation::new(head_type);
    /// head.insert(tuple! { pc: 0i64 }).unwrap();
    ///
    /// let mut vm = RelationalVM::new(registers, memory, program, head);
    ///
    /// // Run up to 10 steps. It will halt after 1 step since pc=1 has no instruction.
    /// let steps = vm.run(10).unwrap();
    /// assert_eq!(steps, 1);
    /// ```
    pub fn run(&mut self, max_steps: usize) -> Result<usize, DatabaseError> {
        for step in 0..max_steps {
            if !self.step()? {
                return Ok(step);
            }
        }
        Ok(max_steps)
    }
}

#[allow(dead_code)]
#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::values::Tuple;
    use relvar_core::{
        tuple,
        types::{RelationType, TupleType},
    };

    #[test]
    fn test_vm_addition() {
        let reg_heading = TupleType::new()
            .with_attribute("reg_id".to_string(), ScalarType::String)
            .with_attribute("value".to_string(), ScalarType::Int);
        let mut registers = Relation::new(RelationType::new(reg_heading));
        registers
            .insert(tuple! { reg_id: "R1", value: 5i64 })
            .unwrap();
        registers
            .insert(tuple! { reg_id: "R2", value: 10i64 })
            .unwrap();
        registers
            .insert(tuple! { reg_id: "R3", value: 0i64 })
            .unwrap();

        let mem_heading = TupleType::new()
            .with_attribute("address".to_string(), ScalarType::Int)
            .with_attribute("value".to_string(), ScalarType::Int);
        let memory = Relation::new(RelationType::new(mem_heading));

        let prog_heading = TupleType::new()
            .with_attribute("pc".to_string(), ScalarType::Int)
            .with_attribute("opcode".to_string(), ScalarType::String)
            .with_attribute("arg1".to_string(), ScalarType::String)
            .with_attribute("arg2".to_string(), ScalarType::String)
            .with_attribute("arg3".to_string(), ScalarType::String);
        let mut program = Relation::new(RelationType::new(prog_heading));

        // ADD R3, R1, R2
        program
            .insert(tuple! { pc: 0i64, opcode: "ADD", arg1: "R3", arg2: "R1", arg3: "R2" })
            .unwrap();

        let head_heading = TupleType::new().with_attribute("pc".to_string(), ScalarType::Int);
        let mut head = Relation::new(RelationType::new(head_heading));
        head.insert(tuple! { pc: 0i64 }).unwrap();

        let mut vm = RelationalVM::new(registers, memory, program, head);
        let steps = vm.run(10).unwrap();
        assert_eq!(steps, 1);

        let r3 = vm
            .registers
            .restrict(|t: &Tuple| t.get_typed::<String>("reg_id").unwrap() == "R3");
        assert_eq!(r3.cardinality(), 1);
        let r3_val = r3
            .tuples()
            .next()
            .unwrap()
            .get_typed::<i64>("value")
            .unwrap();
        assert_eq!(r3_val, 15);
    }
}
