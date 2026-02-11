use anyhow::{anyhow, bail, Context, Result};
use aptly_aptos::AptosClient;
use aptos_dynamic_transaction_composer::{CallArgument, TransactionComposer};
use clap::Parser;
use move_core_types::{
    account_address::AccountAddress,
    identifier::Identifier,
    int256::{I256, U256},
    language_storage::{ModuleId, TypeTag},
    transaction_argument::TransactionArgument,
    value::MoveValue,
};
use serde::{Deserialize, Deserializer};
use serde_json::{json, Value};
use std::{
    collections::{BTreeSet, HashMap},
    io,
    str::FromStr,
};

const DEFAULT_RPC_URL: &str = "https://api.mainnet.aptoslabs.com/v1";

#[derive(Parser)]
#[command(
    name = "aptos-script-compose",
    about = "Compile Aptos script composer payload JSON from stdin"
)]
struct Cli {
    #[arg(long, default_value = DEFAULT_RPC_URL)]
    rpc_url: String,
    #[arg(long, default_value_t = false)]
    no_metadata: bool,
    #[arg(long, default_value_t = false)]
    emit_script_payload: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct StepInput {
    label: String,
    function: String,
    #[serde(default, rename = "typeArguments", alias = "type_arguments")]
    type_arguments: Vec<String>,
    args: Vec<ArgInput>,
}

#[derive(Debug, Clone)]
enum ArgInput {
    Signer,
    Literal { value: Value },
    Ref { step: String, return_index: usize },
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ArgInputKind {
    Signer,
    Literal,
    Ref,
}

#[derive(Debug, Deserialize)]
struct ArgInputKindOnly {
    kind: ArgInputKind,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SignerArgKind {
    Signer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LiteralArgKind {
    Literal,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RefArgKind {
    Ref,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SignerArgInput {
    #[serde(rename = "kind")]
    _kind: SignerArgKind,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LiteralArgInput {
    #[serde(rename = "kind")]
    _kind: LiteralArgKind,
    value: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RefArgInput {
    #[serde(rename = "kind")]
    _kind: RefArgKind,
    step: String,
    #[serde(rename = "returnIndex", alias = "return_index")]
    return_index: usize,
}

impl<'de> Deserialize<'de> for ArgInput {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = Value::deserialize(deserializer)?;
        let kind = serde_json::from_value::<ArgInputKindOnly>(raw.clone())
            .map_err(serde::de::Error::custom)?
            .kind;

        match kind {
            ArgInputKind::Signer => serde_json::from_value::<SignerArgInput>(raw)
                .map(|SignerArgInput { .. }| ArgInput::Signer)
                .map_err(serde::de::Error::custom),
            ArgInputKind::Literal => serde_json::from_value::<LiteralArgInput>(raw)
                .map(|LiteralArgInput { value, .. }| ArgInput::Literal { value })
                .map_err(serde::de::Error::custom),
            ArgInputKind::Ref => serde_json::from_value::<RefArgInput>(raw)
                .map(
                    |RefArgInput {
                         step, return_index, ..
                     }| { ArgInput::Ref { step, return_index } },
                )
                .map_err(serde::de::Error::custom),
        }
    }
}

#[derive(Debug, Clone)]
struct ResolvedStep {
    label: String,
    function_id: FunctionId,
    type_arguments: Vec<String>,
    args: Vec<ArgInput>,
}

#[derive(Debug, Clone)]
struct FunctionId {
    module_id: ModuleId,
    function: String,
}

#[derive(Debug)]
struct ModuleInfo {
    bytecode: Vec<u8>,
    functions: HashMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct RpcModuleResponse {
    bytecode: String,
    #[serde(default)]
    abi: Option<RpcModuleAbi>,
}

#[derive(Debug, Deserialize)]
struct RpcModuleAbi {
    #[serde(default)]
    exposed_functions: Vec<RpcFunctionAbi>,
}

#[derive(Debug, Deserialize)]
struct RpcFunctionAbi {
    name: String,
    #[serde(default)]
    params: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SerializedScriptOutput {
    code: Vec<u8>,
    #[serde(default)]
    ty_args: Vec<TypeTag>,
    #[serde(default)]
    args: Vec<TransactionArgument>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    run(cli)
}

fn run(cli: Cli) -> Result<()> {
    let payload_steps = read_payload_from_stdin()?;
    let steps = resolve_steps(payload_steps)?;
    let required_modules = collect_required_modules(&steps)?;

    let client = AptosClient::new(&cli.rpc_url)?;
    let mut composer = TransactionComposer::single_signer();
    let mut modules = HashMap::new();

    for module_id in required_modules {
        let module_info = fetch_module_info(&client, &module_id)?;
        composer
            .store_module(module_info.bytecode.clone())
            .map_err(|err| anyhow!("failed to load module {} into composer: {err}", module_id))?;
        modules.insert(module_id, module_info);
    }

    let mut returns_by_label: HashMap<String, Vec<CallArgument>> = HashMap::new();
    let mut payload_arguments: Vec<Value> = Vec::new();
    for step in steps {
        let module_info = modules
            .get(&step.function_id.module_id)
            .ok_or_else(|| anyhow!("module {} was not loaded", step.function_id.module_id))?;
        let expected_params = resolve_function_params(&step, module_info)?;
        if expected_params.len() != step.args.len() {
            bail!(
                "step `{}` argument count mismatch: function expects {}, payload provides {}",
                step.label,
                expected_params.len(),
                step.args.len()
            );
        }

        let mut args = Vec::with_capacity(step.args.len());
        for (index, (arg, expected_param)) in
            step.args.iter().zip(expected_params.iter()).enumerate()
        {
            let call_arg = match arg {
                ArgInput::Signer => {
                    let expected = normalize_type_name(expected_param);
                    if expected != "&signer" {
                        bail!(
                            "step `{}` arg {} uses `signer` but expected parameter type is `{}`",
                            step.label,
                            index,
                            expected_param
                        );
                    }
                    CallArgument::new_signer(0)
                }
                ArgInput::Literal { value } => {
                    let bytes = encode_literal(expected_param, value).with_context(|| {
                        format!(
                            "failed to encode literal for step `{}` arg {} (expected `{}`)",
                            step.label, index, expected_param
                        )
                    })?;
                    payload_arguments.push(
                        normalize_literal_for_script_payload(expected_param, value).with_context(
                            || {
                                format!(
                                    "failed to normalize literal for script payload in step `{}` arg {}",
                                    step.label, index
                                )
                            },
                        )?,
                    );
                    CallArgument::new_bytes(bytes)
                }
                ArgInput::Ref {
                    step: ref_step,
                    return_index,
                } => {
                    let values = returns_by_label.get(ref_step).ok_or_else(|| {
                        anyhow!(
                            "step `{}` arg {} references unknown step `{}`",
                            step.label,
                            index,
                            ref_step
                        )
                    })?;
                    let value = values.get(*return_index).ok_or_else(|| {
                        anyhow!(
                            "step `{}` arg {} references `{}` return index {} but only {} return value(s) are available",
                            step.label,
                            index,
                            ref_step,
                            return_index,
                            values.len()
                        )
                    })?;
                    value.clone()
                }
            };
            args.push(call_arg);
        }

        let returns = composer
            .add_batched_call(
                step.function_id.module_string(),
                step.function_id.function.clone(),
                step.type_arguments.clone(),
                args,
            )
            .with_context(|| {
                format!(
                    "composer rejected step `{}` ({})",
                    step.label,
                    step.function_id.fully_qualified()
                )
            })?;
        returns_by_label.insert(step.label, returns);
    }

    let script_bytes = composer
        .generate_batched_calls(!cli.no_metadata)
        .map_err(|err| anyhow!("failed to generate batched script: {err}"))?;

    if cli.emit_script_payload {
        let script: SerializedScriptOutput =
            bcs::from_bytes(&script_bytes).context("failed to decode generated script output")?;
        if script.args.len() != payload_arguments.len() {
            bail!(
                "generated script argument count mismatch: script has {} argument(s), normalized payload has {}",
                script.args.len(),
                payload_arguments.len()
            );
        }
        let type_arguments: Vec<String> = script
            .ty_args
            .iter()
            .map(TypeTag::to_canonical_string)
            .collect();
        let payload = json!({
            "type": "script_payload",
            "code": {
                "bytecode": format!("0x{}", hex::encode(script.code))
            },
            "type_arguments": type_arguments,
            "arguments": payload_arguments
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("0x{}", hex::encode(script_bytes));
    }

    Ok(())
}

fn read_payload_from_stdin() -> Result<Vec<StepInput>> {
    let stdin = io::stdin();
    let raw: Value = serde_json::from_reader(stdin.lock())
        .context("failed to parse script compose payload JSON from stdin")?;
    parse_steps_payload(raw)
}

fn parse_steps_payload(raw: Value) -> Result<Vec<StepInput>> {
    match raw {
        Value::Array(_) => serde_json::from_value::<Vec<StepInput>>(raw)
            .context("failed to parse script compose payload as step array"),
        _ => bail!("invalid payload shape: expected top-level step array `[...]`"),
    }
}

fn resolve_steps(payload_steps: Vec<StepInput>) -> Result<Vec<ResolvedStep>> {
    if payload_steps.is_empty() {
        bail!("payload must include at least one step");
    }

    let mut resolved = Vec::with_capacity(payload_steps.len());
    let mut labels: HashMap<String, usize> = HashMap::new();

    for (index, step) in payload_steps.into_iter().enumerate() {
        let label = step.label.trim().to_owned();
        if label.is_empty() {
            bail!("step at index {index} has an empty label");
        }
        if labels.contains_key(&label) {
            bail!("duplicate step label `{label}`");
        }

        let function_id = FunctionId::parse(&step.function)
            .with_context(|| format!("invalid function id in step `{label}`"))?;

        for type_argument in &step.type_arguments {
            TypeTag::from_str(type_argument).with_context(|| {
                format!("invalid type argument `{type_argument}` in step `{label}`")
            })?;
        }

        for (arg_index, arg) in step.args.iter().enumerate() {
            if let ArgInput::Ref { step: ref_step, .. } = arg {
                if !labels.contains_key(ref_step) {
                    bail!(
                        "step `{label}` arg {} references `{}`. refs must point to a previous step label",
                        arg_index,
                        ref_step
                    );
                }
            }
        }

        labels.insert(label.clone(), index);
        resolved.push(ResolvedStep {
            label,
            function_id,
            type_arguments: step.type_arguments,
            args: step.args,
        });
    }

    Ok(resolved)
}

fn collect_required_modules(steps: &[ResolvedStep]) -> Result<BTreeSet<ModuleId>> {
    let mut modules = BTreeSet::new();
    for step in steps {
        modules.insert(step.function_id.module_id.clone());
        for type_argument in &step.type_arguments {
            let tag = TypeTag::from_str(type_argument).with_context(|| {
                format!(
                    "invalid type argument `{type_argument}` in step `{}`",
                    step.label
                )
            })?;
            for visited in tag.preorder_traversal_iter() {
                if let Some(struct_tag) = visited.struct_tag() {
                    modules.insert(struct_tag.module_id());
                }
            }
        }
    }
    Ok(modules)
}

fn fetch_module_info(client: &AptosClient, module_id: &ModuleId) -> Result<ModuleInfo> {
    let address = module_id.address().to_hex_literal();
    let module_name = module_id.name().as_str();
    let encoded_module = urlencoding::encode(module_name);
    let path = format!("/accounts/{address}/module/{encoded_module}");
    let value = client
        .get_json(&path)
        .with_context(|| format!("failed to fetch module {} via {}", module_id, path))?;
    let module: RpcModuleResponse = serde_json::from_value(value)
        .with_context(|| format!("unexpected module response format for {}", module_id))?;

    let bytecode_hex = module
        .bytecode
        .strip_prefix("0x")
        .unwrap_or(module.bytecode.as_str());
    let bytecode = hex::decode(bytecode_hex)
        .with_context(|| format!("failed to decode bytecode for module {}", module_id))?;

    let mut functions = HashMap::new();
    if let Some(abi) = module.abi {
        for function in abi.exposed_functions {
            functions.insert(function.name, function.params);
        }
    }

    Ok(ModuleInfo {
        bytecode,
        functions,
    })
}

fn resolve_function_params(step: &ResolvedStep, module_info: &ModuleInfo) -> Result<Vec<String>> {
    let params = module_info
        .functions
        .get(&step.function_id.function)
        .ok_or_else(|| {
            anyhow!(
                "function `{}` was not found in module ABI for {}",
                step.function_id.function,
                step.function_id.module_id
            )
        })?;

    let mut resolved = Vec::with_capacity(params.len());
    for param in params {
        let substituted = substitute_type_parameters(param, &step.type_arguments);
        if contains_unresolved_type_param(&substituted) {
            bail!(
                "function parameter `{}` still has unresolved generic placeholders after applying type arguments {:?}",
                param,
                step.type_arguments
            );
        }
        resolved.push(substituted);
    }
    Ok(resolved)
}

fn substitute_type_parameters(param: &str, type_arguments: &[String]) -> String {
    let chars: Vec<char> = param.chars().collect();
    let mut resolved = String::with_capacity(param.len());
    let mut i = 0;

    while i < chars.len() {
        if let Some((digits_start, end)) = type_param_placeholder_span(&chars, i) {
            let index: Option<usize> = chars[digits_start..end]
                .iter()
                .collect::<String>()
                .parse::<usize>()
                .ok();
            if let Some(type_arg) = index.and_then(|value| type_arguments.get(value)) {
                resolved.push_str(type_arg);
            } else {
                for ch in &chars[i..end] {
                    resolved.push(*ch);
                }
            }
            i = end;
            continue;
        }

        resolved.push(chars[i]);
        i += 1;
    }

    resolved
}

fn contains_unresolved_type_param(param: &str) -> bool {
    let chars: Vec<char> = param.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if let Some((_, end)) = type_param_placeholder_span(&chars, i) {
            if end > i {
                return true;
            }
            i = end;
            continue;
        }
        i += 1;
    }

    false
}

fn type_param_placeholder_span(chars: &[char], start: usize) -> Option<(usize, usize)> {
    if chars.get(start).copied()? != 'T' {
        return None;
    }
    if !chars.get(start + 1).is_some_and(char::is_ascii_digit) {
        return None;
    }

    let prev_ok = start == 0 || !is_type_param_ident_char(chars[start - 1]);
    if !prev_ok {
        return None;
    }

    let mut end = start + 1;
    while chars.get(end).is_some_and(char::is_ascii_digit) {
        end += 1;
    }

    let next_ok = end == chars.len() || !is_type_param_ident_char(chars[end]);
    if !next_ok {
        return None;
    }

    Some((start + 1, end))
}

fn is_type_param_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn encode_literal(expected_param: &str, value: &Value) -> Result<Vec<u8>> {
    let mut expected = normalize_type_name(expected_param);
    if expected.starts_with("&mut") {
        expected = expected.trim_start_matches("&mut").to_owned();
    } else if expected.starts_with('&') {
        expected = expected.trim_start_matches('&').to_owned();
    }

    if expected.contains('&') {
        bail!("unsupported literal parameter type `{expected_param}`");
    }

    match expected.as_str() {
        "bool" => serialize_move_value(MoveValue::Bool(parse_bool_literal(value)?)),
        "u8" => serialize_move_value(MoveValue::U8(parse_number(value, "u8")?)),
        "u16" => serialize_move_value(MoveValue::U16(parse_number(value, "u16")?)),
        "u32" => serialize_move_value(MoveValue::U32(parse_number(value, "u32")?)),
        "u64" => serialize_move_value(MoveValue::U64(parse_number(value, "u64")?)),
        "u128" => serialize_move_value(MoveValue::U128(parse_number(value, "u128")?)),
        "u256" => serialize_move_value(MoveValue::U256(parse_number(value, "u256")?)),
        "i8" => serialize_move_value(MoveValue::I8(parse_number(value, "i8")?)),
        "i16" => serialize_move_value(MoveValue::I16(parse_number(value, "i16")?)),
        "i32" => serialize_move_value(MoveValue::I32(parse_number(value, "i32")?)),
        "i64" => serialize_move_value(MoveValue::I64(parse_number(value, "i64")?)),
        "i128" => serialize_move_value(MoveValue::I128(parse_number(value, "i128")?)),
        "i256" => serialize_move_value(MoveValue::I256(parse_number(value, "i256")?)),
        "address" => serialize_move_value(MoveValue::Address(parse_address_literal(value)?)),
        "vector<u8>" => serialize_move_value(MoveValue::vector_u8(parse_bytes_literal(value)?)),
        _ if is_object_type(&expected) => {
            // Object<T> is a single-field wrapper over address.
            serialize_move_value(MoveValue::Address(parse_address_literal(value)?))
        }
        _ if is_string_wrapper_type(&expected) => {
            let string = parse_string_literal(value)?;
            serialize_move_value(MoveValue::vector_u8(string.into_bytes()))
        }
        _ => bail!("unsupported literal parameter type `{expected_param}`"),
    }
}

fn normalize_literal_for_script_payload(expected_param: &str, value: &Value) -> Result<Value> {
    let mut expected = normalize_type_name(expected_param);
    if expected.starts_with("&mut") {
        expected = expected.trim_start_matches("&mut").to_owned();
    } else if expected.starts_with('&') {
        expected = expected.trim_start_matches('&').to_owned();
    }

    if expected.contains('&') {
        bail!("unsupported script payload literal parameter type `{expected_param}`");
    }

    match expected.as_str() {
        "bool" => Ok(Value::Bool(parse_bool_literal(value)?)),
        "u8" => Ok(json!(parse_number::<u8>(value, "u8")?)),
        "u16" => Ok(json!(parse_number::<u16>(value, "u16")?)),
        "u32" => Ok(json!(parse_number::<u32>(value, "u32")?)),
        "u64" => Ok(Value::String(
            parse_number::<u64>(value, "u64")?.to_string(),
        )),
        "u128" => Ok(Value::String(
            parse_number::<u128>(value, "u128")?.to_string(),
        )),
        "u256" => Ok(Value::String(
            parse_number::<U256>(value, "u256")?.to_string(),
        )),
        "i8" => Ok(json!(parse_number::<i8>(value, "i8")?)),
        "i16" => Ok(json!(parse_number::<i16>(value, "i16")?)),
        "i32" => Ok(json!(parse_number::<i32>(value, "i32")?)),
        "i64" => Ok(Value::String(
            parse_number::<i64>(value, "i64")?.to_string(),
        )),
        "i128" => Ok(Value::String(
            parse_number::<i128>(value, "i128")?.to_string(),
        )),
        "i256" => Ok(Value::String(
            parse_number::<I256>(value, "i256")?.to_string(),
        )),
        "address" => Ok(Value::String(
            parse_address_literal(value)?.to_hex_literal(),
        )),
        "vector<u8>" => Ok(Value::String(format!(
            "0x{}",
            hex::encode(parse_bytes_literal(value)?)
        ))),
        _ if is_object_type(&expected) => Ok(Value::String(
            parse_address_literal(value)?.to_hex_literal(),
        )),
        _ if is_string_wrapper_type(&expected) => Ok(Value::String(parse_string_literal(value)?)),
        _ => bail!("unsupported literal parameter type `{expected_param}`"),
    }
}

fn normalize_type_name(value: &str) -> String {
    value.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn is_object_type(value: &str) -> bool {
    value.starts_with("0x1::object::Object<") && value.ends_with('>')
}

fn is_string_wrapper_type(value: &str) -> bool {
    matches!(value, "0x1::string::String" | "0x1::ascii::String")
}

fn parse_bool_literal(value: &Value) -> Result<bool> {
    value
        .as_bool()
        .ok_or_else(|| anyhow!("expected boolean literal"))
}

fn parse_string_literal(value: &Value) -> Result<String> {
    let text = value
        .as_str()
        .ok_or_else(|| anyhow!("expected string literal"))?;
    Ok(text.to_owned())
}

fn parse_bytes_literal(value: &Value) -> Result<Vec<u8>> {
    match value {
        Value::String(s) => {
            let text = s.trim();
            let hex_value = text
                .strip_prefix("0x")
                .ok_or_else(|| anyhow!("vector<u8> string literal must be hex with 0x prefix"))?;
            hex::decode(hex_value).context("failed to decode vector<u8> hex literal")
        }
        Value::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, item)| {
                parse_number::<u8>(item, "u8").with_context(|| {
                    format!("vector<u8> element at index {} is not a valid u8", index)
                })
            })
            .collect(),
        _ => Err(anyhow!(
            "expected vector<u8> literal as 0x-prefixed string or array of u8"
        )),
    }
}

fn parse_address_literal(value: &Value) -> Result<AccountAddress> {
    let raw = value
        .as_str()
        .ok_or_else(|| anyhow!("expected address literal as string"))?
        .trim();
    if raw.starts_with("0x") {
        return AccountAddress::from_hex_literal(raw)
            .or_else(|_| AccountAddress::from_str(raw))
            .with_context(|| format!("invalid address literal `{raw}`"));
    }
    AccountAddress::from_str(raw).with_context(|| format!("invalid address literal `{raw}`"))
}

fn parse_number<T>(value: &Value, type_name: &str) -> Result<T>
where
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Display,
{
    let text = normalize_numeric_literal(value)?;
    text.parse::<T>()
        .map_err(|err| anyhow!("invalid {type_name} literal `{text}`: {err}"))
}

fn normalize_numeric_literal(value: &Value) -> Result<String> {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                bail!("empty numeric string literal");
            }
            Ok(trimmed.strip_suffix('n').unwrap_or(trimmed).to_owned())
        }
        Value::Number(number) => Ok(number.to_string()),
        _ => bail!("expected numeric literal as number or string"),
    }
}

fn serialize_move_value(value: MoveValue) -> Result<Vec<u8>> {
    value
        .simple_serialize()
        .ok_or_else(|| anyhow!("failed to BCS-encode literal value"))
}

impl FunctionId {
    fn parse(input: &str) -> Result<Self> {
        let parts: Vec<&str> = input.split("::").collect();
        if parts.len() != 3 {
            bail!(
                "function id must be `<address>::<module>::<function>`, got `{}`",
                input
            );
        }

        let module = format!("{}::{}", parts[0], parts[1]);
        let module_id =
            ModuleId::from_str(&module).with_context(|| format!("invalid module id `{module}`"))?;
        let function = Identifier::new(parts[2])
            .map_err(|_| anyhow!("invalid function identifier `{}`", parts[2]))?
            .to_string();
        Ok(Self {
            module_id,
            function,
        })
    }

    fn fully_qualified(&self) -> String {
        format!("{}::{}", self.module_string(), self.function)
    }

    fn module_string(&self) -> String {
        format!(
            "{}::{}",
            self.module_id.address().to_hex_literal(),
            self.module_id.name()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_tokens_field() {
        let json = r#"
        {
          "tokens": [],
          "steps": []
        }
        "#;
        let raw: Value = serde_json::from_str(json).unwrap();
        assert!(parse_steps_payload(raw).is_err());
    }

    #[test]
    fn parses_direct_steps_array_with_type_arguments_alias() {
        let json = r#"
        [{
            "label": "s1",
            "function": "0x1::coin::withdraw",
            "type_arguments": ["0x1::aptos_coin::AptosCoin"],
            "args": [{"kind":"signer"}, {"kind":"literal","value":"1"}]
        }]
        "#;
        let raw: Value = serde_json::from_str(json).unwrap();
        let steps = parse_steps_payload(raw).unwrap();
        assert_eq!(steps[0].type_arguments.len(), 1);
    }

    #[test]
    fn rejects_legacy_steps_object() {
        let json = r#"
        {
          "steps": [{
            "label": "s1",
            "function": "0x1::coin::withdraw",
            "args": [{"kind":"signer"}, {"kind":"literal","value":"1"}]
          }]
        }
        "#;
        let raw: Value = serde_json::from_str(json).unwrap();
        assert!(parse_steps_payload(raw).is_err());
    }

    #[test]
    fn rejects_unknown_fields_in_args() {
        let json = r#"
        [{
            "label": "s1",
            "function": "0x1::coin::withdraw",
            "args": [{"kind":"signer","foo":1}]
        }]
        "#;
        let raw: Value = serde_json::from_str(json).unwrap();
        assert!(parse_steps_payload(raw).is_err());
    }

    #[test]
    fn substitutes_generic_placeholders() {
        let actual = substitute_type_parameters(
            "0x1::object::Object<T0>",
            &["0x1::fungible_asset::Metadata".to_owned()],
        );
        assert_eq!(actual, "0x1::object::Object<0x1::fungible_asset::Metadata>");
    }

    #[test]
    fn substitutes_generics_without_touching_identifier_text() {
        let actual = substitute_type_parameters(
            "vector<T1>",
            &["u8".to_owned(), "0x1::my::T0Coin".to_owned()],
        );
        assert_eq!(actual, "vector<0x1::my::T0Coin>");
    }

    #[test]
    fn substitutes_only_token_placeholders() {
        let actual = substitute_type_parameters("0x1::my::T0Coin<T0>", &["u8".to_owned()]);
        assert_eq!(actual, "0x1::my::T0Coin<u8>");
    }

    #[test]
    fn encodes_u64_with_bigint_suffix() {
        let bytes = encode_literal("u64", &Value::String("205000000n".to_owned())).unwrap();
        let expected = MoveValue::U64(205_000_000).simple_serialize().unwrap();
        assert_eq!(bytes, expected);
    }

    #[test]
    fn encodes_object_as_address() {
        let value = Value::String("0x1".to_owned());
        let bytes = encode_literal("0x1::object::Object<T0>", &value).unwrap();
        let expected = MoveValue::Address(AccountAddress::ONE)
            .simple_serialize()
            .unwrap();
        assert_eq!(bytes, expected);
    }

    #[test]
    fn normalizes_u64_for_script_payload() {
        let value =
            normalize_literal_for_script_payload("u64", &Value::String("205000000n".into()))
                .unwrap();
        assert_eq!(value, Value::String("205000000".to_owned()));
    }

    #[test]
    fn normalizes_object_for_script_payload() {
        let value = normalize_literal_for_script_payload(
            "0x1::object::Object<T0>",
            &Value::String("0x1".into()),
        )
        .unwrap();
        assert_eq!(value, Value::String("0x1".to_owned()));
    }
}
