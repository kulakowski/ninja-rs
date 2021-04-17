use crate::arena;
use crate::blob;
use crate::blob::{Blob, Builder};
use crate::lex;

#[derive(Debug)]
pub enum AstError {
    DuplicateBinding,
}

pub struct Declarations {
    declarations: Vec<Declaration>,
}

impl Declarations {
    pub fn new() -> Declarations {
        Declarations {
            declarations: vec![],
        }
    }

    pub fn add_rule(&mut self, rule: Rule) -> Result<(), AstError> {
        self.declarations.push(Declaration::Rule(rule));
        Ok(())
    }

    pub fn add_build(&mut self, build: Build) -> Result<(), AstError> {
        self.declarations.push(Declaration::Build(build));
        Ok(())
    }

    pub fn add_default(&mut self, default: Default) -> Result<(), AstError> {
        self.declarations.push(Declaration::Default(default));
        Ok(())
    }

    pub fn add_pool(&mut self, pool: Pool) -> Result<(), AstError> {
        self.declarations.push(Declaration::Pool(pool));
        Ok(())
    }

    pub fn count(&self) -> usize {
        self.declarations.len()
    }
}

pub struct File {
    declarations: Declarations,
    scopes: Scopes,
}

impl File {
    pub fn new(declarations: Declarations, scopes: Scopes) -> File {
        File {
            declarations,
            scopes,
        }
    }

    pub fn declarations(&self) -> &Declarations {
        &self.declarations
    }

    pub fn declarations_mut(&mut self) -> &mut Declarations {
        &mut self.declarations
    }
}

pub enum Declaration {
    Rule(Rule),
    Build(Build),
    Default(Default),
    Pool(Pool),
}

pub struct Rule {
    name: lex::Identifier,
    scope: arena::Id<Scope>,
}

impl Rule {
    pub fn new(name: lex::Identifier, scope: arena::Id<Scope>) -> Rule {
        Rule { name, scope }
    }
}

pub struct Build {
    outputs: Vec<Target>,
    implicit_outputs: Vec<Target>,
    rule: lex::Identifier,
    inputs: Vec<Target>,
    implicit_inputs: Vec<Target>,
    order_inputs: Vec<Target>,
    scope: arena::Id<Scope>,
}

impl Build {
    pub fn new(
        outputs: Vec<Target>,
        implicit_outputs: Vec<Target>,
        rule: lex::Identifier,
        inputs: Vec<Target>,
        implicit_inputs: Vec<Target>,
        order_inputs: Vec<Target>,
        scope: arena::Id<Scope>,
    ) -> Build {
        Build {
            outputs,
            implicit_outputs,
            rule,
            inputs,
            implicit_inputs,
            order_inputs,
            scope,
        }
    }
}

pub struct Default {
    targets: Vec<Target>,
}

impl Default {
    pub fn new(targets: Vec<Target>) -> Default {
        Default { targets }
    }
}

pub struct Pool {
    name: lex::Identifier,
    depth: usize,
}

impl Pool {
    pub fn new(name: lex::Identifier, depth: usize) -> Pool {
        Pool { name, depth }
    }
}

pub struct Value {
    value: lex::Value,
}

impl Value {
    pub fn new(value: lex::Value) -> Value {
        Value { value }
    }
}

pub struct Target {
    value: lex::Value,
}

impl Target {
    pub fn new(value: lex::Value) -> Target {
        Target { value }
    }
}

pub struct Scopes {
    arena: arena::Arena<Scope>,
    top: arena::Id<Scope>,
}

impl Scopes {
    pub fn new() -> Scopes {
        let mut arena = arena::Arena::new();
        let top = arena.insert(Scope::empty(None));
        Scopes { arena, top }
    }

    pub fn new_scope(&mut self, bindings: Vec<Binding>) -> Result<arena::Id<Scope>, AstError> {
        let scope = Scope::new(bindings, Some(self.top))?;
        let id = self.arena.insert(scope);
        Ok(id)
    }

    pub fn top(&self) -> arena::Id<Scope> {
        self.top
    }

    pub fn get_scope(&self, id: arena::Id<Scope>) -> &Scope {
        self.arena.get(id)
    }

    pub fn get_scope_mut(&mut self, id: arena::Id<Scope>) -> &mut Scope {
        self.arena.get_mut(id)
    }

    pub fn get(&self, mut id: arena::Id<Scope>, identifier: lex::Identifier) -> Option<&[u8]> {
        loop {
            let scope = self.get_scope(id);
            match scope.get(identifier) {
                Some(value) => return Some(value),
                None => match scope.parent {
                    Some(parent) => id = parent,
                    None => return None,
                },
            }
        }
    }
}

pub struct Binding {
    id: lex::Identifier,
    value: Blob,
}

impl Binding {
    pub fn new(id: lex::Identifier, value: Blob) -> Binding {
        Binding { id, value }
    }
}

pub struct Scope {
    bindings: std::collections::HashMap<lex::Identifier, Blob>,
    parent: Option<arena::Id<Scope>>,
}

impl Scope {
    pub fn empty(parent: Option<arena::Id<Scope>>) -> Scope {
        let bindings = std::collections::HashMap::new();
        Scope { bindings, parent }
    }

    pub fn new(
        new_bindings: Vec<Binding>,
        parent: Option<arena::Id<Scope>>,
    ) -> Result<Scope, AstError> {
        let mut scope = Scope::empty(parent);

        for binding in new_bindings {
            scope.push(binding)?;
        }

        Ok(scope)
    }

    pub fn push(&mut self, binding: Binding) -> Result<(), AstError> {
        if self.bindings.insert(binding.id, binding.value).is_some() {
            Err(AstError::DuplicateBinding)
        } else {
            Ok(())
        }
    }

    pub fn get(&self, identifier: lex::Identifier) -> Option<&blob::View> {
        self.bindings.get(&identifier).map(|v| v.as_ref())
    }

    pub fn size(&self) -> usize {
        self.bindings.len()
    }

    pub fn evaluate(&self, value: &Value) -> Blob {
        let mut builder = Builder::new();
        for part in value.value.parts.iter() {
            match part {
                lex::ValuePart::Text(text) => builder.extend(text),
                lex::ValuePart::Variable(variable) => {
                    let text = self.get(*variable).unwrap_or(b"");
                    builder.extend(text);
                }
            }
        }
        builder.blob()
    }
}
