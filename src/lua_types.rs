use std::any::Any;
use hexagon_vm_core::object::Object;
use hexagon_vm_core::value::Value;

pub struct Pair {
    left: Value,
    right: Value
}

impl Object for Pair {
    fn get_children(&self) -> Vec<usize> {
        let mut ret: Vec<usize> = Vec::new();
        if let Value::Object(id) = self.left {
            ret.push(id);
        }
        if let Value::Object(id) = self.right {
            ret.push(id);
        }
        ret
    }

    fn as_any(&self) -> &Any {
        self as &Any
    }

    fn as_any_mut(&mut self) -> &mut Any {
        self as &mut Any
    }
}
