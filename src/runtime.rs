use hexagon_vm_core::executor::ExecutorImpl;
use hexagon_vm_core::value::{Value, ValueContext};
use hexagon_vm_core::builtin::array::Array;
use hexagon_vm_core::object::Object;
use hexagon_vm_core::builtin::dynamic_object::DynamicObject;
use hexagon_vm_core::function::Function;
use hexagon_vm_core::errors::VMError;
use codegen::ModuleBuilder;
use lua_types::Table;

pub struct ModuleRuntime<'a> {
    executor: &'a mut ExecutorImpl
}

macro_rules! alloc_object {
    ($e:expr, $v:expr) => (Value::Object($e.get_object_pool_mut().allocate(
        Box::new($v)
    )))
}

macro_rules! native {
    ($e:expr, $f:expr) => (alloc_object!($e, Function::from_native(Box::new($f))))
}

macro_rules! set_fields {
    ( $g:ident, $($k:expr => $v:expr),* ) => {
        {
            $(
                $g.set_field(
                    $k,
                    $v
                );
            )*
        }
    }
}

fn init_global_resources(e: &mut ExecutorImpl, g: &mut DynamicObject) {
    set_fields!(
        g,
        "print" => native!(e, |e| {
            let v = e.get_current_frame().must_get_argument(0);
            let s = ValueContext::new(&v, e.get_object_pool()).to_str().to_string();
            println!("{}", s);
            Value::Null
        }),
        "assert" => native!(e, |e| {
            let v = e.get_current_frame().must_get_argument(0);
            let cond = ValueContext::new(&v, e.get_object_pool()).to_bool();
            if !cond {
                panic!(if let Some(reason) = e.get_current_frame().get_argument(1) {
                    let reason = ValueContext::new(&reason, e.get_object_pool());;
                    VMError::from(format!("Assertion failed: {}", reason.to_str()))
                } else {
                    VMError::from("Assertion failed")
                });
            }
            Value::Null
        }),
        "@__luax_internal.new_table" => native!(e, |e| {
            alloc_object!(e, Table::new())
        }),
        "panic" => Value::Null
    );
}

pub fn invoke(executor: &mut ExecutorImpl, builder: ModuleBuilder, entry_fn_id: usize) {
    let functions = builder.functions.into_inner();
    let mut global_resources = DynamicObject::new(None);

    let mut fn_res = Array::new();
    let mut local_fn_res: Vec<Value> = Vec::new();

    for mut f in functions {
        f.enable_optimization();
        let f_obj = Value::Object(
            executor.get_object_pool_mut().allocate(Box::new(f))
        );
        fn_res.elements.borrow_mut().push(f_obj);
        local_fn_res.push(f_obj);
    }

    let target: Value = (*fn_res.elements.borrow())[entry_fn_id];

    global_resources.set_field(
        "@__luax_internal.functions",
        Value::Object(executor.get_object_pool_mut().allocate(Box::new(fn_res)))
    );

    init_global_resources(executor, &mut global_resources);

    //global_resources.freeze();

    let global_resources_inst = Value::Object(executor.get_object_pool_mut().allocate(
        Box::new(global_resources)
    ));

    for f in local_fn_res {
        if let Value::Object(id) = f {
            let f = executor.get_object_pool().must_get_typed::<Function>(id);
            f.bind_this(global_resources_inst);
            f.static_optimize(executor.get_object_pool_mut());
            if let Some(info) = f.to_virtual_info() {
                println!("{:?}", info);
            }
        } else {
            unreachable!()
        }
    }

    executor.invoke(target, Value::Null, None, &[]);
}
