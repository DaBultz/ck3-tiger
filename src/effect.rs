use std::fmt::{Display, Formatter};

use crate::block::validator::Validator;
use crate::block::Block;
use crate::context::ScopeContext;
use crate::data::scriptvalues::ScriptValue;
use crate::errorkey::ErrorKey;
use crate::errors::{error, warn};
use crate::everything::Everything;
use crate::item::Item;
use crate::scopes::{scope_iterator, scope_prefix, scope_to_scope, Scopes};
use crate::tables::effects::{scope_effect, Effect};
use crate::trigger::{validate_normal_trigger, validate_target};
use crate::validate::{validate_inside_iterator, validate_prefix_reference};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ListType {
    None,
    Any,
    Every,
    Ordered,
    Random,
}

impl Display for ListType {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        match self {
            ListType::None => write!(f, ""),
            ListType::Any => write!(f, "any"),
            ListType::Every => write!(f, "every"),
            ListType::Ordered => write!(f, "ordered"),
            ListType::Random => write!(f, "random"),
        }
    }
}

pub fn validate_normal_effect<'a>(
    block: &Block,
    data: &'a Everything,
    sc: &mut ScopeContext,
    tooltipped: bool,
) {
    let vd = Validator::new(block, data);
    validate_effect("", ListType::None, block, data, sc, vd, tooltipped);
}

pub fn validate_effect<'a>(
    caller: &str,
    list_type: ListType,
    block: &Block,
    data: &'a Everything,
    mut sc: &mut ScopeContext,
    mut vd: Validator<'a>,
    mut tooltipped: bool,
) {
    // undocumented
    if let Some(key) = block.get_key("custom") {
        vd.field_value_item("custom", Item::Localization);
        if list_type == ListType::None {
            warn(
                key,
                ErrorKey::Validation,
                "`custom` can only be used in lists",
            );
        }
        tooltipped = false;
    }

    if let Some(b) = vd.field_block("limit") {
        if caller == "if" || caller == "else_if" || caller == "else" || list_type != ListType::None
        {
            validate_normal_trigger(b, data, sc, tooltipped);
        } else {
            warn(
                block.get_key("limit").unwrap(),
                ErrorKey::Validation,
                "`limit` can only be used in if/else_if or lists",
            );
        }
    }

    vd.field_validated_blocks("alternative_limit", |b, data| {
        if list_type != ListType::None {
            validate_normal_trigger(b, data, sc, false);
        } else {
            warn(
                b,
                ErrorKey::Validation,
                "`alternative_limit` can only be used in lists",
            );
        }
    });

    if let Some(bv) = vd.field("order_by") {
        if list_type == ListType::Ordered {
            ScriptValue::validate_bv(bv, data, sc);
        } else {
            warn(
                block.get_key("order_by").unwrap(),
                ErrorKey::Validation,
                "`order_by` can only be used in `ordered_` lists",
            );
        }
    }

    if let Some(token) = vd.field_value("position") {
        if list_type == ListType::Ordered {
            if token.as_str().parse::<i32>().is_err() {
                warn(token, ErrorKey::Validation, "expected an integer");
            }
        } else {
            warn(
                block.get_key("position").unwrap(),
                ErrorKey::Validation,
                "`position` can only be used in `ordered_` lists",
            );
        }
    }

    if let Some(token) = vd.field_value("min") {
        if list_type == ListType::Ordered {
            if token.as_str().parse::<i32>().is_err() {
                warn(token, ErrorKey::Validation, "expected an integer");
            }
        } else {
            warn(
                block.get_key("min").unwrap(),
                ErrorKey::Validation,
                "`min` can only be used in `ordered_` lists",
            );
        }
    }

    if let Some(bv) = vd.field("max") {
        if list_type == ListType::Ordered {
            ScriptValue::validate_bv(bv, data, sc);
        } else {
            warn(
                block.get_key("max").unwrap(),
                ErrorKey::Validation,
                "`max` can only be used in `ordered_` lists",
            );
        }
    }

    if let Some(token) = vd.field_value("check_range_bounds") {
        if list_type == ListType::Ordered {
            if !(token.is("yes") || token.is("no")) {
                warn(token, ErrorKey::Validation, "expected yes or no");
            }
        } else {
            warn(
                block.get_key("check_range_bounds").unwrap(),
                ErrorKey::Validation,
                "`check_range_bounds` can only be used in `ordered_` lists",
            );
        }
    }

    if let Some(_b) = vd.field_block("weight") {
        if list_type == ListType::Random {
            // TODO
        } else {
            warn(
                block.get_key("weight").unwrap(),
                ErrorKey::Validation,
                "`weight` can only be used in `random_` lists",
            );
        }
    }

    validate_inside_iterator(
        caller,
        &list_type.to_string(),
        block,
        data,
        sc,
        &mut vd,
        tooltipped,
    );

    'outer: for (key, bv) in vd.unknown_keys() {
        if let Some((inscopes, effect)) = scope_effect(key, data) {
            sc.expect(inscopes, key.clone());
            match effect {
                Effect::Yes => {
                    if let Some(token) = bv.expect_value() {
                        if !token.is("yes") {
                            let msg = format!("expected just `{} = yes`", key);
                            warn(token, ErrorKey::Validation, &msg);
                        }
                    }
                }
                Effect::Bool => {
                    if let Some(token) = bv.expect_value() {
                        validate_target(token, data, sc, Scopes::Bool);
                    }
                }
                Effect::Integer => {
                    if let Some(token) = bv.expect_value() {
                        if token.as_str().parse::<i32>().is_err() {
                            warn(token, ErrorKey::Validation, "expected integer");
                        }
                    }
                }
                Effect::Value | Effect::ScriptValue | Effect::NonNegativeValue => {
                    if let Some(token) = bv.get_value() {
                        if let Ok(number) = token.as_str().parse::<i32>() {
                            if effect == Effect::NonNegativeValue && number < 0 {
                                let msg = format!("{} does not take negative numbers", key);
                                warn(token, ErrorKey::Validation, &msg);
                            }
                        }
                    }
                    ScriptValue::validate_bv(bv, data, sc);
                }
                Effect::Scope(outscopes) => {
                    if let Some(token) = bv.expect_value() {
                        validate_target(token, data, sc, outscopes);
                    }
                }
                Effect::Item(itype) => {
                    if let Some(token) = bv.expect_value() {
                        data.verify_exists(itype, token);
                    }
                }
                Effect::Unchecked => (),
                _ => (),
            }
            continue;
        }

        if let Some((it_type, it_name)) = key.split_once('_') {
            if it_type.is("any")
                || it_type.is("ordered")
                || it_type.is("every")
                || it_type.is("random")
            {
                if let Some((inscopes, outscope)) = scope_iterator(&it_name, data) {
                    if it_type.is("any") {
                        let msg = "cannot use `any_` lists in an effect";
                        error(key, ErrorKey::Validation, msg);
                        continue;
                    }
                    sc.expect(inscopes, key.clone());
                    let ltype = match it_type.as_str() {
                        "every" => ListType::Every,
                        "ordered" => ListType::Ordered,
                        "random" => ListType::Random,
                        _ => unreachable!(),
                    };
                    sc.open_scope(outscope, key.clone());
                    if let Some(b) = bv.expect_block() {
                        let vd = Validator::new(b, data);
                        validate_effect(it_name.as_str(), ltype, b, data, sc, vd, tooltipped);
                    }
                    sc.close();
                    continue;
                }
            }
        }

        if data.item_exists(Item::ScriptedEffect, key.as_str()) || data.events.effect_exists(key) {
            // TODO: validate macros
            if let Some(token) = bv.get_value() {
                if !token.is("yes") {
                    warn(token, ErrorKey::Validation, "expected just effect = yes");
                }
            }
            // If it's a block, then it should contain macro arguments
            continue;
        }

        // Check if it's a target = { target_scope } block.
        // The logic here is similar to logic in triggers and script values,
        // but not quite the same :(
        let part_vec = key.split('.');
        sc.open_builder();
        for i in 0..part_vec.len() {
            let first = i == 0;
            let part = &part_vec[i];

            if let Some((prefix, arg)) = part.split_once(':') {
                if let Some((inscopes, outscope)) = scope_prefix(prefix.as_str()) {
                    if inscopes == Scopes::None && !first {
                        let msg = format!("`{}:` makes no sense except as first part", prefix);
                        warn(part, ErrorKey::Validation, &msg);
                    }
                    sc.expect(inscopes, prefix.clone());
                    validate_prefix_reference(&prefix, &arg, data);
                    sc.replace(outscope, part.clone());
                } else {
                    let msg = format!("unknown prefix `{}:`", prefix);
                    error(part, ErrorKey::Validation, &msg);
                    sc.close();
                    continue 'outer;
                }
            } else if part.is("root")
                || part.is("prev")
                || part.is("this")
                || part.is("ROOT")
                || part.is("PREV")
                || part.is("THIS")
            {
                if !first {
                    let msg = format!("`{}` makes no sense except as first part", part);
                    warn(part, ErrorKey::Validation, &msg);
                }
                if part.is("root") || part.is("ROOT") {
                    sc.replace_root();
                } else if part.is("prev") || part.is("PREV") {
                    sc.replace_prev(&part);
                } else {
                    sc.replace_this();
                }
            } else if let Some((inscopes, outscope)) = scope_to_scope(part.as_str()) {
                if inscopes == Scopes::None && !first {
                    let msg = format!("`{}` makes no sense except as first part", part);
                    warn(part, ErrorKey::Validation, &msg);
                }
                sc.expect(inscopes, part.clone());
                sc.replace(outscope, part.clone());
            // TODO: warn if trying to use iterator or effect here
            } else {
                let msg = format!("unknown token `{}`", part);
                error(part, ErrorKey::Validation, &msg);
                sc.close();
                continue 'outer;
            }
        }

        if let Some(block) = bv.expect_block() {
            validate_normal_effect(block, data, &mut sc, tooltipped);
        }
        sc.close();
    }

    vd.warn_remaining();
}
