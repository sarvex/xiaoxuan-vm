// Copyright (c) 2022 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use anvm_parser::{
    ast::{FunctionType, LocalGroup},
    instruction::Instruction,
    types::Value,
};

use crate::{
    instance::{EngineError, Function},
    interpreter,
    vm_module::VMModule,
};

pub struct VMFunction {
    function_type: FunctionType,
    function_item: FunctionItem,
}

pub enum FunctionItem {
    /// 内部函数
    ///
    /// 当前模块内定义的函数
    Internal {
        local_groups: Vec<LocalGroup>,
        expression: Rc<Vec<Instruction>>,
        vm_module: Weak<RefCell<VMModule>>,
    },

    /// 外部函数
    ///
    /// 即来自其他模块的函数，因为一个模块可以从其他模块导入函数，
    /// 所有在模块的函数列表里，即包含在当前模块
    /// 内部定义的 `内部函数` 函数，也包含从外部导入的 `外部函数`。
    External(Rc<dyn Function>),
}

impl VMFunction {
    pub fn new_internal_function(
        function_type: FunctionType,
        local_groups: Vec<LocalGroup>,
        expression: Rc<Vec<Instruction>>,
        vm_module: Weak<RefCell<VMModule>>,
    ) -> Self {
        VMFunction {
            function_type,
            function_item: FunctionItem::Internal {
                local_groups,
                expression,
                vm_module,
            },
        }
    }

    pub fn new_external_function(function_type: FunctionType, r: Rc<dyn Function>) -> Self {
        VMFunction {
            function_type,
            function_item: FunctionItem::External(r),
        }
    }
}

impl Function for VMFunction {
    /// 从 vm 外部调用函数
    fn eval(&self, args: &[Value]) -> Result<Vec<Value>, EngineError> {
        match &self.function_item {
            FunctionItem::Internal {
                local_groups,
                expression,
                vm_module,
            } => {
                let rc = match vm_module.upgrade() {
                    Some(rc) => rc,
                    _ => panic!("failed to get the module instance"),
                };

                let mut vm = rc.borrow_mut();
                eval_internal_function(&self.function_type, local_groups, expression, &mut vm, args)
            }
            FunctionItem::External(r) => {
                // 对于 `外部函数`，使用它自己的 eval() 方法求值，
                // 也就是说它会作为其所在的模块的 `内部函数` 来求值。
                r.as_ref().eval(args)
            }
        }
    }

    fn get_function_type(&self) -> FunctionType {
        self.function_type.clone()
    }
}

/// 从 vm 外部调用模块内部定义的函数
fn eval_internal_function(
    function_type: &FunctionType,
    local_groups: &Vec<LocalGroup>,
    expression: &Vec<Instruction>,
    vm_module: &mut VMModule,
    args: &[Value],
) -> Result<Vec<Value>, EngineError> {
    push_args(vm_module, function_type, args)?;
    interpreter::call_internal_function(vm_module, function_type, local_groups, expression);
    vm_module.do_loop();

    Ok(pop_results(vm_module, function_type))
}

/// 从 vm 外部调用模块内部函数时，将入的实参压入操作数栈
///
/// 参数列表左边（小索引端）的实参先压入
/// 对于返回结果，先弹出的数值放置在结果数组的右边（大索引端）
///
/// 示例：
/// caller     caller
///  |          ^
///  V          |
/// (a,b,c)    (x,y)
///  | | |      ^ ^
///  V V V      | |
/// internal function
///
/// |--- 栈顶。---|   |--- 栈顶。---|
/// | - c        |   |            |
/// | - b        |   | - y        |
/// | - a        |   | - x        |
/// | - ...      |   | - ...      |
/// |--- 栈底。---|   |--- 栈底。---|
fn push_args(
    vm_module: &mut VMModule,
    function_type: &FunctionType,
    args: &[Value],
) -> Result<(), EngineError> {
    if args.len() != function_type.params.len() {
        return Err(EngineError::InvalidOperation(
            "the number of arguments and parameters do not match".to_string(),
        ));
    }

    vm_module.operand_stack.push_values(args);
    Ok(())
}

fn pop_results(vm_module: &mut VMModule, function_type: &FunctionType) -> Vec<Value> {
    let count = function_type.results.len();
    vm_module.operand_stack.pop_values(count)
}