use std::error::Error;
use std::fmt;
use hexagon_vm_core::opcode::OpCode;
use ast::{Block, Expr, Stmt, Lhs, GetUsedVars};
use codegen::{ModuleBuilder, FunctionBuilder, BasicBlockBuilder, LoopControlInfo, VarLocation};

#[derive(Debug)]
pub struct CodegenError {
    desc: String
}

impl<'a> From<&'a str> for CodegenError {
    fn from(other: &'a str) -> CodegenError {
        CodegenError {
            desc: other.to_string()
        }
    }
}

impl Default for CodegenError {
    fn default() -> Self {
        CodegenError {
            desc: "Error while generating code".into()
        }
    }
}

impl Error for CodegenError {
    fn description(&self) -> &str {
        self.desc.as_str()
    }
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CodegenError: {}", self.desc)
    }
}

impl Lhs {
    fn build_set(&self, fb: &mut FunctionBuilder) -> Result<(), CodegenError> {
        match *self {
            Lhs::Id(ref id) => {
                let loc = fb.get_var_location(id);
                loc.build_set(fb)?;
            },
            Lhs::Index(ref target, ref index) => {
                index.restricted_generate_code(fb)?;
                target.restricted_generate_code(fb)?;
                fb.write_index_set()?;
            }
        }
        Ok(())
    }

    fn build_new_local(&self, fb: &mut FunctionBuilder) -> Result<(), CodegenError> {
        match *self {
            Lhs::Id(ref id) => {
                let loc = fb.create_local(id);
                loc.build_set(fb)?;
            },
            _ => return Err("build_new_local: Unexpected lvalue".into())
        }
        Ok(())
    }
}

pub trait RestrictedGenerateCode {
    fn restricted_generate_code(&self, fb: &mut FunctionBuilder) -> Result<(), CodegenError>;
}

pub trait UnrestrictedGenerateCode {
    fn unrestricted_generate_code(&self, fb: &mut FunctionBuilder) -> Result<(), CodegenError>;
}

impl UnrestrictedGenerateCode for Block {
    fn unrestricted_generate_code(&self, fb: &mut FunctionBuilder) -> Result<(), CodegenError> {
        for stmt in self.statements() {
            stmt.unrestricted_generate_code(fb)?;
        }

        Ok(())
    }
}

impl UnrestrictedGenerateCode for Stmt {
    fn unrestricted_generate_code(&self, fb: &mut FunctionBuilder) -> Result<(), CodegenError> {
        match *self {
            Stmt::Do(ref stmts) => {
                for stmt in stmts {
                    stmt.unrestricted_generate_code(fb)?;
                }
            },
            Stmt::Set(ref lhs, ref exprs) => {
                if lhs.len() != exprs.len() {
                    return Err("Set: lhs & exprs length mismatch".into());
                }
                for i in 0..lhs.len() {
                    exprs[i].restricted_generate_code(fb)?;
                    lhs[i].build_set(fb)?;
                }
            },
            Stmt::While(ref expr, ref blk) => {
                fb.scoped(|fb| -> Result<(), CodegenError> {
                    let expr_check_bb_id = fb.current_basic_block + 1;
                    fb.get_current_bb().opcodes.push(OpCode::Branch(expr_check_bb_id));
                    fb.move_forward();

                    expr.restricted_generate_code(fb)?;

                    let break_point_bb_id = fb.current_basic_block + 1;
                    fb.move_forward();

                    let body_begin_bb_id = fb.current_basic_block + 1;
                    fb.move_forward();

                    fb.with_lci(LoopControlInfo {
                        break_point: break_point_bb_id,
                        continue_point: expr_check_bb_id
                    }, |fb| blk.unrestricted_generate_code(fb))?;

                    fb.get_current_bb().opcodes.push(OpCode::Branch(expr_check_bb_id));

                    let end_bb_id = fb.current_basic_block + 1;
                    fb.move_forward();

                    fb.basic_blocks[break_point_bb_id].opcodes.push(OpCode::Branch(end_bb_id));
                    fb.basic_blocks[expr_check_bb_id].opcodes.push(OpCode::ConditionalBranch(
                        body_begin_bb_id,
                        end_bb_id
                    ));
                    Ok(())
                })?;
            },
            Stmt::Local(ref lhs, ref exprs) => {
                if lhs.len() != exprs.len() {
                    return Err("Local: lhs & exprs length mismatch".into());
                }
                for i in 0..lhs.len() {
                    exprs[i].restricted_generate_code(fb)?;
                    lhs[i].build_new_local(fb)?;
                }
            },
            Stmt::Call(ref target, ref args) => {
                Expr::Call(Box::new(target.clone()), args.clone()).restricted_generate_code(fb)?;
                fb.get_current_bb().opcodes.push(OpCode::Pop);
            },
            Stmt::Return(ref v) => {
                if v.len() == 0 {
                    fb.get_current_bb().opcodes.push(OpCode::LoadNull);
                    fb.get_current_bb().opcodes.push(OpCode::Return);
                    fb.move_forward();
                } else if v.len() == 1 {
                    v[0].restricted_generate_code(fb)?;
                    fb.get_current_bb().opcodes.push(OpCode::Return);
                    fb.move_forward();
                } else {
                    return Err("Multiple return values is not supported for now".into());
                }
            },
            _ => return Err("Not implemented".into())
        }

        Ok(())
    }
}

impl RestrictedGenerateCode for Expr {
    fn restricted_generate_code(&self, fb: &mut FunctionBuilder) -> Result<(), CodegenError> {
        match *self {
            Expr::Nil => fb.get_current_bb().opcodes.push(OpCode::LoadNull),
            Expr::Boolean(v) => fb.get_current_bb().opcodes.push(OpCode::LoadBool(v)),
            Expr::Number(v) => fb.get_current_bb().opcodes.push(OpCode::LoadFloat(v)),
            Expr::String(ref s) => fb.get_current_bb().opcodes.push(OpCode::LoadString(s.clone())),
            Expr::Function(ref vlhs, ref blk) => {
                let mut new_builder = fb.get_module_builder().new_function();

                let mut arg_names: Vec<String> = Vec::new();
                for lhs in vlhs {
                    if let Some(id) = lhs.id() {
                        arg_names.push(id.to_string());
                    } else {
                        return Err("Expecting id in function signature".into());
                    }
                }

                new_builder.build_args_load(arg_names)?;
                let fn_id = new_builder.build(blk)?;

                fb.write_function_load(fn_id)?;
            },
            Expr::Table(ref elems) => {
                fb.write_table_create()?;
                for v in elems {
                    fb.get_current_bb().opcodes.push(OpCode::Dup);
                    v.restricted_generate_code(fb)?;
                    fb.get_current_bb().opcodes.push(OpCode::Rotate2);
                    fb.write_table_set()?;
                }
            },
            Expr::Add(ref left, ref right) => {
                left.restricted_generate_code(fb)?;
                right.restricted_generate_code(fb)?;
                fb.get_current_bb().opcodes.push(OpCode::Rotate2);
                fb.get_current_bb().opcodes.push(OpCode::Add);
            },
            Expr::Sub(ref left, ref right) => {
                left.restricted_generate_code(fb)?;
                right.restricted_generate_code(fb)?;
                fb.get_current_bb().opcodes.push(OpCode::Rotate2);
                fb.get_current_bb().opcodes.push(OpCode::Sub);
            },
            Expr::Mul(ref left, ref right) => {
                left.restricted_generate_code(fb)?;
                right.restricted_generate_code(fb)?;
                fb.get_current_bb().opcodes.push(OpCode::Rotate2);
                fb.get_current_bb().opcodes.push(OpCode::Mul);
            },
            Expr::Div(ref left, ref right) => {
                left.restricted_generate_code(fb)?;
                right.restricted_generate_code(fb)?;
                fb.get_current_bb().opcodes.push(OpCode::Rotate2);
                fb.get_current_bb().opcodes.push(OpCode::Div);
            },
            Expr::Idiv(ref left, ref right) => {
                left.restricted_generate_code(fb)?;
                right.restricted_generate_code(fb)?;
                fb.get_current_bb().opcodes.push(OpCode::Rotate2);
                fb.get_current_bb().opcodes.push(OpCode::IntDiv);
            },
            Expr::Mod(ref left, ref right) => {
                left.restricted_generate_code(fb)?;
                right.restricted_generate_code(fb)?;
                fb.get_current_bb().opcodes.push(OpCode::Rotate2);
                fb.get_current_bb().opcodes.push(OpCode::Mod);
            },
            Expr::Pow(ref left, ref right) => {
                left.restricted_generate_code(fb)?;
                right.restricted_generate_code(fb)?;
                fb.get_current_bb().opcodes.push(OpCode::Rotate2);
                fb.get_current_bb().opcodes.push(OpCode::Pow);
            },
            Expr::Concat(ref left, ref right) => {
                left.restricted_generate_code(fb)?;
                right.restricted_generate_code(fb)?;
                fb.get_current_bb().opcodes.push(OpCode::Rotate2);
                fb.write_concat()?;
            },
            Expr::Eq(ref left, ref right) => {
                left.restricted_generate_code(fb)?;
                right.restricted_generate_code(fb)?;
                fb.get_current_bb().opcodes.push(OpCode::Rotate2);
                fb.get_current_bb().opcodes.push(OpCode::TestEq);
            },
            Expr::Ne(ref left, ref right) => {
                left.restricted_generate_code(fb)?;
                right.restricted_generate_code(fb)?;
                fb.get_current_bb().opcodes.push(OpCode::Rotate2);
                fb.get_current_bb().opcodes.push(OpCode::TestNe);
            },
            Expr::Lt(ref left, ref right) => {
                left.restricted_generate_code(fb)?;
                right.restricted_generate_code(fb)?;
                fb.get_current_bb().opcodes.push(OpCode::Rotate2);
                fb.get_current_bb().opcodes.push(OpCode::TestLt);
            },
            Expr::Gt(ref left, ref right) => {
                left.restricted_generate_code(fb)?;
                right.restricted_generate_code(fb)?;
                fb.get_current_bb().opcodes.push(OpCode::Rotate2);
                fb.get_current_bb().opcodes.push(OpCode::TestGt);
            },
            Expr::Le(ref left, ref right) => {
                left.restricted_generate_code(fb)?;
                right.restricted_generate_code(fb)?;
                fb.get_current_bb().opcodes.push(OpCode::Rotate2);
                fb.get_current_bb().opcodes.push(OpCode::TestLe);
            },
            Expr::Ge(ref left, ref right) => {
                left.restricted_generate_code(fb)?;
                right.restricted_generate_code(fb)?;
                fb.get_current_bb().opcodes.push(OpCode::Rotate2);
                fb.get_current_bb().opcodes.push(OpCode::TestGe);
            },
            Expr::Not(ref v) => {
                v.restricted_generate_code(fb)?;
                fb.get_current_bb().opcodes.push(OpCode::Not);
            },
            Expr::Call(ref target, ref args) => {
                for arg in args {
                    arg.restricted_generate_code(fb)?;
                }
                fb.get_current_bb().opcodes.push(OpCode::RotateReverse(args.len()));
                fb.get_current_bb().opcodes.push(OpCode::LoadNull);
                target.restricted_generate_code(fb)?;
                fb.get_current_bb().opcodes.push(OpCode::Call(args.len()));
            },
            Expr::Pair(ref left, ref right) => {
                left.restricted_generate_code(fb)?;
                right.restricted_generate_code(fb)?;
                fb.get_current_bb().opcodes.push(OpCode::Rotate2);
                fb.write_pair_create()?;
            },
            Expr::Id(ref k) => {
                let v = match fb.get_module_builder().lookup_var(k.as_str()) {
                    Some(v) => v,
                    None => VarLocation::This(k.clone())
                };
                v.build_get(fb)?;
            },
            Expr::Index(ref target, ref index) => {
                index.restricted_generate_code(fb)?;
                target.restricted_generate_code(fb)?;
                fb.write_index_get()?;
            },
            Expr::Dots => {
                return Err("Dots: Not implemented".into());
            }
        }

        Ok(())
    }
}
