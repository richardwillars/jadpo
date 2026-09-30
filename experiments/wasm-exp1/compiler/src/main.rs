//! Experiment-only checked-model projection; neither a Wasm route nor a runtime.
use jadpo_core::{analyze_project, checked_source_revision, derive_target, AnalyzedProject};
use jadpo_syntax::*;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, String>;
fn name(path: &[Name]) -> String {
    path.iter()
        .map(|v| v.text.as_str())
        .collect::<Vec<_>>()
        .join(".")
}
fn tag<T: std::fmt::Debug>(value: T) -> String {
    format!("{value:?}")
}
fn literal(value: &Literal) -> Result<Value> {
    if value.kind == LiteralKind::None {
        return Ok(Value::Null);
    }
    serde_json::from_str(&value.text).map_err(|_| {
        format!(
            "WASM_PROJECTION_LITERAL: unsupported literal encoding at {}..{}",
            value.range.start, value.range.end
        )
    })
}
struct Projection<'a> {
    project: &'a AnalyzedProject,
    root: &'a Path,
    source: &'a str,
}
impl<'a> Projection<'a> {
    fn span(&self, range: TextRange) -> Value {
        json!({"source":Path::new(self.source).strip_prefix(self.root).unwrap_or(Path::new(self.source)).to_string_lossy(),"start":range.start,"end":range.end})
    }
    fn reject(&self, range: TextRange, reason: &str) -> String {
        format!(
            "WASM_PROJECTION_UNSUPPORTED: {reason}; {}",
            self.span(range)
        )
    }
    fn constraints(&self, values: &[Constraint]) -> Result<Value> {
        Ok(Value::Array(values.iter().map(|c|Ok(json!({"kind":tag(c.kind),"value":literal(&c.value)?,"span":self.span(c.range)}))).collect::<Result<_>>()?))
    }
    fn reference(&self, r: &TypeReference) -> Result<Value> {
        let n = name(&r.path);
        if !r.arguments.is_empty() {
            return Err(self.reject(
                r.range,
                "generic collections outside frozen scalar/record projection",
            ));
        }
        Ok(
            json!({"name":n,"nullable":r.nullable||self.project.semantics.nullable_types.contains(&n),"authoredNullable":r.nullable,"span":self.span(r.range)}),
        )
    }
    fn fields(&self, values: &[FieldDeclaration]) -> Result<Value> {
        Ok(Value::Array(values.iter().map(|f| {
      if f.generated.is_some() {return Err(self.reject(f.range,"generated lifecycle field outside frozen slice"));}
      Ok(json!({"name":f.name.text,"type":self.reference(&f.field_type)?,"optional":f.optional,"constraints":self.constraints(&f.constraints)?,"immutable":f.immutable,"storage":f.persistence.iter().map(|v|tag(v)).collect::<Vec<_>>(),"reference":f.reference.as_ref().map(|r|json!({"target":name(&r.target.path),"onDelete":tag(r.on_delete)})),"role":f.role.as_ref().map(|r|name(&r.role.path)),"span":self.span(f.range)}))
    }).collect::<Result<_>>()?))
    }
    fn initializers(&self, fields: &[FieldInitialiser]) -> Result<Value> {
        Ok(Value::Array(fields.iter().map(|f|Ok(json!({"name":f.name.text,"value":self.expression(&f.value)?,"span":self.span(f.range)}))).collect::<Result<_>>()?))
    }
    fn rejection(&self, r: &RejectStatement) -> Result<Value> {
        Ok(
            json!({"op":"reject","failure":r.failure.text,"fields":self.initializers(&r.values)?,"span":self.span(r.range)}),
        )
    }
    fn conflicts(&self, c: &[ConflictBinding]) -> Result<Value> {
        Ok(Value::Array(c.iter().map(|c|Ok(json!({"constraint":c.constraint.as_ref().map(|n|name(&n.path)),"failure":self.rejection(&c.rejection)?}))).collect::<Result<_>>()?))
    }
    fn expression(&self, e: &Expression) -> Result<Value> {
        let mut v = match e {
            Expression::Literal(l) => {
                json!({"op":"literal","kind":tag(l.kind),"value":literal(l)?})
            }
            Expression::Name(n) => {
                json!({"op":"load","path":n.path.iter().map(|p|&p.text).collect::<Vec<_>>()})
            }
            Expression::Invocation(i) => {
                let target = name(&i.callee.path);
                let node = self.project.semantics.node(&target).ok_or_else(|| {
                    self.reject(
                        i.range,
                        "unresolved invocation; receiver inference not in frozen subset",
                    )
                })?;
                if !matches!(
                    node.kind.as_str(),
                    "function" | "action" | "query" | "type" | "field"
                ) {
                    return Err(self.reject(
                        i.range,
                        "non-callable/nominal invocation outside projection",
                    ));
                }
                if !i.named_arguments.is_empty() {
                    return Err(self.reject(i.range, "named arguments outside frozen subset"));
                }
                json!({"op":if matches!(node.kind.as_str(),"type"|"field"){"constructScalar"}else{"call"},"target":target,"targetId":node.id.0,"arguments":i.arguments.iter().map(|a|self.expression(a)).collect::<Result<Vec<_>>>()?})
            }
            Expression::Construction(c) => {
                json!({"op":"constructRecord","target":name(&c.target.path),"fields":self.initializers(&c.fields)?})
            }
            Expression::Object(o) => json!({"op":"object","fields":self.initializers(&o.fields)?}),
            Expression::Attempt(a) => json!({"op":"attempt","value":self.expression(&a.value)?}),
            Expression::OutcomeMatch(m) => {
                json!({"op":"outcomeMatch","subject":self.expression(&m.subject)?,"arms":m.arms.iter().map(|arm| {
        let pattern=match &arm.pattern {OutcomeMatchPattern::Success(n)=>json!({"kind":"success","binding":n.text}),OutcomeMatchPattern::Failure(n)=>json!({"kind":"failure","failure":n.text})};
        let body=match &arm.body {OutcomeMatchArmBody::Value(e)=>self.expression(e)?,OutcomeMatchArmBody::Reject(r)=>self.rejection(r)?,OutcomeMatchArmBody::Propagate(_)=>json!({"op":"propagate"})};
        Ok(json!({"pattern":pattern,"body":body,"span":self.span(arm.range)}))
      }).collect::<Result<Vec<_>>>()?})
            }
            Expression::Query(q) => {
                if !q.includes.is_empty()
                    || q.order.is_some()
                    || q.pagination.is_some()
                    || q.cardinality != QueryCardinality::Required
                {
                    return Err(self.reject(
                        q.range,
                        "only required single-row scalar-key reads in frozen projection",
                    ));
                }
                json!({"op":"query","cardinality":"required","entity":name(&q.target.path),"predicate":{"field":q.field.text,"operator":"equal","value":self.expression(&q.value)?},"missing":q.missing.as_ref().map(|r|self.rejection(r)).transpose()?,"hostEffect":"storage.read","policy":"checkedOperationObligations"})
            }
            Expression::Update(u) => {
                if !u.conditional_changes.is_empty() {
                    return Err(self.reject(
                        u.range,
                        "conditional patch derivation outside frozen subset",
                    ));
                }
                json!({"op":"update","entity":name(&u.target.path),"predicate":{"field":u.field.text,"operator":"equal","value":self.expression(&u.value)?},"set":self.initializers(&u.changes)?,"patch":u.patch.as_ref().map(|p|p.path.iter().map(|n|n.text.clone()).collect::<Vec<_>>()),"empty":u.empty.as_ref().map(|r|self.rejection(r)).transpose()?,"missing":self.rejection(&u.missing)?,"conflicts":self.conflicts(&u.conflicts)?,"hostEffect":"storage.update","policy":"checkedOperationObligations"})
            }
            Expression::Binary(b) => {
                json!({"op":"binary","operator":tag(b.operator),"left":self.expression(&b.left)?,"right":self.expression(&b.right)?})
            }
            Expression::Unary(u) => {
                json!({"op":"unary","operator":tag(u.operator),"value":self.expression(&u.value)?})
            }
            Expression::Grouped(g) => return self.expression(&g.value),
            _ => return Err(self.reject(e.range(), "expression kind outside frozen projection")),
        };
        v["span"] = self.span(e.range());
        v["type"] = self
            .project
            .typing
            .expressions
            .iter()
            .find(|x| x.source == self.source && x.range == e.range())
            .map(|x| json!(x.type_name))
            .unwrap_or(Value::Null);
        Ok(v)
    }
    fn block(&self, b: &Block) -> Result<Value> {
        Ok(Value::Array(b.statements.iter().map(|s|{
    let mut v=match s {
      Statement::Binding(b)=>json!({"op":"bind","name":b.name.text,"mutable":b.mutable,"annotation":b.annotation.as_ref().map(|t|self.reference(t)).transpose()?,"value":self.expression(&b.value)?}),
      Statement::Return(r)=>json!({"op":"return","value":self.expression(&r.value)?}),
      Statement::Reject(r)=>self.rejection(r)?,
      Statement::If(i)=>json!({"op":"if","condition":self.expression(&i.condition)?,"then":self.block(&i.then_block)?,"else":i.else_block.as_ref().map(|b|self.block(b)).transpose()?}),
      _=>return Err(self.reject(s.range(),"statement kind outside frozen projection")),
    }; v["span"]=self.span(s.range());Ok(v)
 }).collect::<Result<_>>()?))
    }
    fn declaration(&self, d: &Declaration) -> Result<Value> {
        let mut v=match d {
      Declaration::Type(t)=>json!({"kind":"type","name":t.name.text,"parent":self.reference(&t.parent)?,"constraints":self.constraints(&t.constraints)?,"span":self.span(t.range)}),
      Declaration::Record(r)=>{
        if r.membership.is_some()||!r.inverses.is_empty()||!r.persistence_constraints.is_empty(){return Err(self.reject(r.range,"memberships/inverses/compound constraints outside frozen projection"));}
        if r.dossier.as_ref().is_some_and(|d|!d.representations.is_empty()){return Err(self.reject(r.range,"derived stores outside frozen projection"));}
        json!({"kind":"record","recordKind":tag(r.kind),"name":r.name.text,"persistent":r.is_persistent_entity(),"fields":self.fields(&r.fields)?,"span":self.span(r.range)})
      },
      Declaration::Enum(e)=>{
        if e.variants.iter().any(|v|!v.fields.is_empty()){return Err(self.reject(e.range,"tagged enums outside frozen projection"));}
        json!({"kind":"enum","name":e.name.text,"variants":e.variants.iter().map(|v|&v.name.text).collect::<Vec<_>>(),"span":self.span(e.range)})
      },
      Declaration::Failure(f)=>{
        let contract=self.project.failures.contracts.iter().find(|c|c.name==f.name.text).ok_or("missing checked failure contract")?;
        json!({"kind":"failure","name":f.name.text,"category":contract.kind,"code":contract.code,"message":contract.message,"httpStatus":contract.http_status,"public":self.fields(&f.public_fields)?,"internal":self.fields(&f.internal_fields)?,"span":self.span(f.range)})
      },
      Declaration::Callable(c)=>{
        if c.receiver.is_some()||c.mutation_guard.is_some()||matches!(c.consistency,Some(ConsistencyDisposition::DurableWorkflow)) {return Err(self.reject(c.range,"receiver/revision/durable workflow outside frozen subset"));}
        if c.freshness.is_some()&&c.freshness!=Some(QueryFreshness::Authoritative){return Err(self.reject(c.range,"only authoritative named reads have a frozen host contract"));}
        let effect=self.project.failures.callables.iter().find(|e|e.callable==c.name.text).ok_or("missing checked callable failure set")?;
        json!({"kind":"callable","callableKind":tag(c.kind),"name":c.name.text,"owner":c.owner.as_ref().map(|o|&o.text),"parameters":c.parameters.iter().map(|p|Ok(json!({"name":p.name.text,"type":self.reference(&p.parameter_type)?}))).collect::<Result<Vec<_>>>()?,"result":self.reference(&c.return_type)?,"failures":effect.failures,"maySuspend":effect.may_suspend,"consistency":c.consistency.map(tag),"freshness":c.freshness.map(tag),"body":self.block(&c.body)?,"span":self.span(c.range)})
      },
      _=>return Err("WASM_PROJECTION_UNSUPPORTED: declaration outside fixture projection (routes/auth/config/fixtures must not silently lower)".into()),
    };
        if let Some(n) = v.get("name").and_then(Value::as_str) {
            if let Some(node) = self.project.semantics.node(n) {
                v["semanticId"] = json!(node.id.0);
                v["operationId"] = json!(format!("{}:{}", node.kind.as_str(), node.name));
            }
        }
        Ok(v)
    }
}
fn effective_types(declarations: &[Value]) -> Result<Value> {
    let mut authored = BTreeMap::<String, Value>::new();
    for d in declarations {
        let name = d["name"].as_str().unwrap();
        if d["kind"] == "type" {
            authored.insert(
                name.into(),
                json!({"parent":d["parent"],"constraints":d["constraints"]}),
            );
        }
        if d["kind"] == "record" {
            for f in d["fields"].as_array().unwrap() {
                authored.insert(
                    format!("{name}.{}", f["name"].as_str().unwrap()),
                    json!({"parent":f["type"],"constraints":f["constraints"]}),
                );
            }
        }
    }
    let mut result = serde_json::Map::new();
    for root in authored.keys() {
        let mut current = root.to_owned();
        let mut constraints = Vec::new();
        let mut nullable = false;
        let mut visited = BTreeSet::new();
        while let Some(t) = authored.get(&current) {
            if !visited.insert(current.clone()) {
                return Err("cycle in checked nominal projection".into());
            }
            constraints.extend(t["constraints"].as_array().unwrap().iter().cloned());
            nullable |= t["parent"]["nullable"].as_bool().unwrap();
            current = t["parent"]["name"].as_str().unwrap().to_owned();
        }
        result.insert(root.clone(),json!({"base":current,"nullable":nullable,"constraints":constraints,"textLengthUnit":"unicodeScalar","nominal":root}));
    }
    Ok(Value::Object(result))
}
fn project(input: &Path) -> Result<(Value, AnalyzedProject)> {
    let project = analyze_project(input).map_err(|d| d.to_json())?;
    for (stage, diagnostics) in [
        ("syntax", project.syntax.diagnostics().collect::<Vec<_>>()),
        ("semantics", project.semantics.diagnostics.iter().collect()),
        ("types", project.typing.diagnostics.iter().collect()),
        ("failures", project.failures.diagnostics.iter().collect()),
    ] {
        if !diagnostics.is_empty() {
            return Err(format!(
                "WASM_PROJECTION_CHECK_FAILED:{stage}: [{}]",
                diagnostics
                    .iter()
                    .map(|d| d.to_json())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
    }
    if !project.policy.memberships.is_empty() {
        return Err("WASM_PROJECTION_UNSUPPORTED: membership policy outside frozen subset".into());
    }
    for tx in &project.entity_model.transactions {
        if tx.disposition == "durable_workflow"
            || tx.domain.as_ref().is_some_and(|d| d != "primary")
        {
            return Err(format!(
                "WASM_PROJECTION_UNSUPPORTED: transaction domain/disposition for {}",
                tx.action
            ));
        }
    }
    let root = if input.is_dir() {
        input
    } else {
        input.parent().unwrap()
    };
    let mut declarations = Vec::new();
    for source in &project.syntax.sources {
        let p = Projection {
            project: &project,
            root,
            source: &source.source_name,
        };
        for d in &source.file.declarations {
            declarations.push(p.declaration(d)?);
        }
    }
    declarations.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    let types = effective_types(&declarations)?;
    let policies = json!({"active":project.policy.active,"bindings":project.policy.bindings.iter().map(|b|json!({"entity":b.entity,"field":b.field,"role":b.role,"scope":b.scope,"principalEntity":b.principal})).collect::<Vec<_>>(),"entities":project.policy.entities.iter().map(|e|json!({"entity":e.entity,"scope":e.scope,"scopeField":e.scope_field,"rules":e.rules.iter().map(|r|json!({"subject":r.subject,"effects":r.effects})).collect::<Vec<_>>(),"fields":e.fields.iter().map(|f|json!({"field":f.field,"rules":f.rules.iter().map(|r|json!({"subject":r.subject,"effects":r.effects})).collect::<Vec<_>>()})).collect::<Vec<_>>()})).collect::<Vec<_>>(),"operations":project.policy.operations.iter().map(|o|json!({"operation":o.operation,"obligations":o.obligations.iter().map(|b|json!({"entity":b.entity,"effect":b.effect,"subjects":b.subjects,"origin":b.source})).collect::<Vec<_>>(),"fieldReads":o.field_reads.iter().map(|f|json!({"entity":f.entity,"field":f.field,"subjects":f.subjects,"origin":f.source})).collect::<Vec<_>>()})).collect::<Vec<_>>()});
    let entities=project.entity_model.entities.iter().map(|e|json!({"name":e.name,"identity":e.identity,"persistent":e.persistent,"authorityStore":e.authority_store,"operations":e.operations})).collect::<Vec<_>>();
    let queries=project.entity_model.queries.iter().map(|q|json!({"name":q.name,"owner":q.owner,"freshness":q.freshness,"plan":q.plan,"reads":q.reads,"predicateFields":q.predicate_fields})).collect::<Vec<_>>();
    let transactions=project.entity_model.transactions.iter().map(|t|json!({"action":t.action,"owners":t.owners,"disposition":t.disposition,"domain":t.domain,"nested":t.nested,"handledFailure":t.handled_failure,"postgresIsolation":t.postgres_isolation,"postgresConcurrency":t.postgres_concurrency,"sqliteIsolation":t.sqlite_isolation,"retry":t.retry})).collect::<Vec<_>>();
    let calls = project
        .semantics
        .calls
        .iter()
        .map(|c| json!({"caller":c.caller.0,"callee":c.callee.0}))
        .collect::<Vec<_>>();
    let value = json!({"schemaVersion":1,"kind":"jadpo_checked_executable_projection","checkedRevision":checked_source_revision(input,&project),"boundary":{"closedRecords":true,"omittedNullPresent":"distinct","textLength":"unicodeScalar","trustedPrincipalOnly":true},"declarations":declarations,"effectiveTypes":types,"policy":policies,"entities":entities,"queries":queries,"transactions":transactions,"callEdges":calls});
    Ok((value, project))
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() < 2 || args.len() > 3 || args.get(2).is_some_and(|v| v != "--bun") {
        return Err("usage: jadpo-wasm-projection PROJECT OUTPUT_DIRECTORY [--bun]".into());
    }
    let input = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);
    let (ir, project) = project(&input)?;
    // Build all output contents before writing: unsupported input must publish none.
    let mut outputs = vec![(
        "program.json".to_owned(),
        serde_json::to_string_pretty(&ir).unwrap() + "\n",
    )];
    if args.len() == 3 {
        let artifacts = derive_target(&input, &project).map_err(|d| d.to_json())?;
        let callables = ir["declarations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["kind"] == "callable")
            .map(|d| d["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        let validators = ir["declarations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| matches!(d["kind"].as_str(), Some("record" | "type" | "enum")))
            .map(|d| d["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        for a in artifacts {
            let mut text = a.contents;
            if a.relative_path == "target/app.ts" {
                text.push_str("\n// Compiler-owned experiment hooks at the trusted test boundary.\nexport const experimentCallables = {\n");
                for n in &callables {
                    text.push_str(&format!(
                        "  {}: {},\n",
                        serde_json::to_string(n).unwrap(),
                        n.replace('.', "__")
                    ));
                }
                text.push_str("};\nexport const experimentValidators = {\n");
                for n in &validators {
                    text.push_str(&format!(
                        "  {}: validate_{},\n",
                        serde_json::to_string(n).unwrap(),
                        n
                    ));
                }
                text.push_str("};\nexport { captureOperation, DomainFailure, ValidationError };\n");
            }
            outputs.push((format!("bun/{}", a.relative_path), text));
        }
    }
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    for (path, text) in outputs {
        let p = output.join(path);
        fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::write(p, text).map_err(|e| e.to_string())?;
    }
    println!(
        "{}",
        json!({"status":"projected","checkedRevision":ir["checkedRevision"],"output":output,"declarations":ir["declarations"].as_array().unwrap().len()})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new(text: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "jadpo-projection-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            fs::write(path.join("app.jadpo"), text).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn source() -> String {
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixture/app.jadpo"))
            .unwrap()
    }
    #[test]
    fn checked_fixture_retains_executable_outcome_policy_and_atomic_plans() {
        let fixture = Fixture::new(&source());
        let (p, _) = project(&fixture.0).unwrap();
        let ds = p["declarations"].as_array().unwrap();
        let probe = ds.iter().find(|d| d["name"] == "probe").unwrap();
        assert_eq!(probe["body"][0]["value"]["op"], "outcomeMatch");
        assert_eq!(
            probe["body"][0]["value"]["subject"]["target"],
            "Item.read_title"
        );
        assert_eq!(
            probe["body"][0]["value"]["arms"][1]["body"]["path"],
            json!(["input", "title"])
        );
        assert!(p["policy"]["bindings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["field"] == "owner_id" && b["principalEntity"] == "User"));
        assert!(p["transactions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["action"] == "update_pair"
                && t["disposition"] == "atomic"
                && t["domain"] == "primary"));
        assert_eq!(p["effectiveTypes"]["ItemPatch.note"]["nullable"], true);
        let input = ds.iter().find(|d| d["name"] == "ProbeInput").unwrap();
        assert!(input["fields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["name"] == "note"
                && f["optional"] == true
                && f["type"]["nullable"] == true));
    }
    #[test]
    fn source_constraint_mutation_changes_projection_and_revision() {
        let original = Fixture::new(&source());
        let changed = Fixture::new(&source().replace("max_length: 12", "max_length: 9"));
        let (a, _) = project(&original.0).unwrap();
        let (b, _) = project(&changed.0).unwrap();
        assert_ne!(a["checkedRevision"], b["checkedRevision"]);
        assert_eq!(
            a["effectiveTypes"]["Item.title"]["constraints"][1]["value"],
            12
        );
        assert_eq!(
            b["effectiveTypes"]["Item.title"]["constraints"][1]["value"],
            9
        );
    }
    #[test]
    fn invalid_checked_source_and_valid_but_unsupported_forms_fail_closed() {
        let invalid = Fixture::new(
            &source().replace("success(title) => title", "success(title) => absent_value"),
        );
        assert!(project(&invalid.0)
            .unwrap_err()
            .starts_with("WASM_PROJECTION_CHECK_FAILED:types:"));
        let unsupported=Fixture::new(&(source()+"\nfunction rebind(value: ItemTitle) -> ItemTitle { var mut result = value\n result = value\n return result }\n"));
        assert!(project(&unsupported.0)
            .unwrap_err()
            .starts_with("WASM_PROJECTION_UNSUPPORTED:"));
    }
}
