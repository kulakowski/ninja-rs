use crate::arena;
use crate::ast;
use crate::blob;
use crate::intern;
use crate::lex;
use crate::lex::{DeclKind, LexError, Lexer, Token, TokenKind};

#[derive(Debug)]
pub enum ParseError {
    LexError(LexError),
    AstError(ast::AstError),
    MissingNewline,
    UnexpectedToken { got: TokenKind },
    UnexpectedEof,
    InvalidValue,
    PoolDepthInvalid,
    Expected { expected: TokenKind, got: TokenKind },
}

pub struct Parser<'input> {
    lexer: Lexer<'input>,
}

impl<'input> Parser<'input> {
    pub fn new(input: &'input blob::View) -> Parser {
        let lexer = Lexer::new(input);
        Parser { lexer }
    }

    pub fn parse(&mut self, arena: &mut intern::Table) -> Result<ast::File, ParseError> {
        let mut declarations = ast::Declarations::new();
        let mut scopes = ast::Scopes::new();

        loop {
            match self.advance_decl()? {
                None => break,
                Some(token) => match token.kind() {
                    DeclKind::Rule => {
                        let rule = self.parse_rule(&mut scopes, arena)?;
                        match declarations.add_rule(rule) {
                            Ok(()) => (),
                            Err(error) => return Err(ParseError::AstError(error)),
                        }
                    }

                    DeclKind::Build => {
                        let build = self.parse_build(&mut scopes, arena)?;
                        match declarations.add_build(build) {
                            Ok(()) => (),
                            Err(error) => return Err(ParseError::AstError(error)),
                        }
                    }

                    DeclKind::Default => {
                        let default = self.parse_default(arena)?;
                        match declarations.add_default(default) {
                            Ok(()) => (),
                            Err(error) => return Err(ParseError::AstError(error)),
                        }
                    }

                    DeclKind::Subninja => {
                        todo!()
                    }

                    DeclKind::Include => {
                        todo!()
                    }

                    DeclKind::Pool => {
                        let pool = self.parse_pool(&mut scopes, arena)?;
                        match declarations.add_pool(pool) {
                            Ok(()) => (),
                            Err(error) => return Err(ParseError::AstError(error)),
                        }
                    }

                    DeclKind::Identifier => {
                        let identifier = lex::Identifier::new(arena, self.lexer.lexeme(token));
                        let binding =
                            self.parse_top_level_binding(&mut scopes, arena, identifier)?;
                        let top = scopes.top();
                        match scopes.get_scope_mut(top).push(binding) {
                            Err(error) => return Err(ParseError::AstError(error)),
                            Ok(()) => (),
                        }
                    }

                    DeclKind::Newline => (),
                },
            }
        }

        Ok(ast::File::new(declarations, scopes))
    }

    fn parse_rule(
        &mut self,
        scopes: &mut ast::Scopes,
        arena: &mut intern::Table,
    ) -> Result<ast::Rule, ParseError> {
        let name = self.parse_identifier(arena)?;
        let _newline = self.consume(TokenKind::Newline)?;
        let scope = self.parse_scope(scopes, arena)?;

        Ok(ast::Rule::new(name, scope))
    }

    fn parse_build(
        &mut self,
        scopes: &mut ast::Scopes,
        arena: &mut intern::Table,
    ) -> Result<ast::Build, ParseError> {
        let mut outputs = vec![];
        let mut implicit_outputs = vec![];
        let mut inputs = vec![];
        let mut implicit_inputs = vec![];
        let mut order_inputs = vec![];

        while let Some(output) = self.parse_target(arena)? {
            outputs.push(output)
        }
        let _colon = match self.advance()? {
            None => return Err(ParseError::UnexpectedEof),
            Some(token) => match token.kind() {
                TokenKind::Pipe => {
                    while let Some(implicit_output) = self.parse_target(arena)? {
                        implicit_outputs.push(implicit_output)
                    }
                    self.consume(TokenKind::Colon)?
                }
                TokenKind::Colon => token,
                got => return Err(ParseError::UnexpectedToken { got }),
            },
        };
        let rule = self.parse_identifier(arena)?;
        while let Some(input) = self.parse_target(arena)? {
            inputs.push(input)
        }
        let _newline = match self.advance()? {
            None => return Err(ParseError::UnexpectedEof),
            Some(token) => match token.kind() {
                TokenKind::Newline => token,
                TokenKind::Pipe => {
                    while let Some(implicit_input) = self.parse_target(arena)? {
                        implicit_inputs.push(implicit_input)
                    }
                    match self.advance()? {
                        None => return Err(ParseError::UnexpectedEof),
                        Some(token) => match token.kind() {
                            TokenKind::Newline => token,
                            TokenKind::PipePipe => {
                                while let Some(order_input) = self.parse_target(arena)? {
                                    order_inputs.push(order_input)
                                }
                                self.consume(TokenKind::Newline)?
                            }
                            got => return Err(ParseError::UnexpectedToken { got }),
                        },
                    }
                }
                TokenKind::PipePipe => {
                    while let Some(order_input) = self.parse_target(arena)? {
                        order_inputs.push(order_input)
                    }
                    self.consume(TokenKind::Newline)?
                }
                got => return Err(ParseError::UnexpectedToken { got }),
            },
        };

        let scope = self.parse_scope(scopes, arena)?;

        Ok(ast::Build::new(
            outputs,
            implicit_outputs,
            rule,
            inputs,
            implicit_inputs,
            order_inputs,
            scope,
        ))
    }

    fn parse_default(&mut self, arena: &mut intern::Table) -> Result<ast::Default, ParseError> {
        let mut targets = vec![];
        while let Some(target) = self.parse_target(arena)? {
            targets.push(target)
        }
        self.consume(TokenKind::Newline)?;
        Ok(ast::Default::new(targets))
    }

    fn parse_pool(
        &mut self,
        scopes: &mut ast::Scopes,
        arena: &mut intern::Table,
    ) -> Result<ast::Pool, ParseError> {
        let name = self.parse_identifier(arena)?;
        let _newline = self.consume(TokenKind::Newline)?;
        let scope_id = self.parse_scope(scopes, arena)?;
        let scope = scopes.get_scope(scope_id);

        if scope.size() != 1 {
            return Err(ParseError::PoolDepthInvalid);
        }
        let depth = lex::Identifier::new(arena, b"depth");
        let depth = match scope.get(depth) {
            None => return Err(ParseError::PoolDepthInvalid),
            Some(depth) => depth,
        };
        let depth = if depth == b"" {
            0
        } else {
            let depth = match String::from_utf8(depth.to_vec()) {
                Ok(depth) => depth,
                _ => return Err(ParseError::PoolDepthInvalid),
            };
            match depth.parse() {
                Err(_) => return Err(ParseError::PoolDepthInvalid),
                Ok(depth) => depth,
            }
        };

        Ok(ast::Pool::new(name, depth))
    }

    fn parse_target(
        &mut self,
        arena: &mut intern::Table,
    ) -> Result<Option<ast::Target>, ParseError> {
        match self.lexer.lex_target(arena) {
            Err(error) => Err(ParseError::LexError(error)),
            Ok(None) => Ok(None),
            Ok(Some(value)) => Ok(Some(ast::Target::new(value))),
        }
    }

    fn parse_scope(
        &mut self,
        scopes: &mut ast::Scopes,
        arena: &mut intern::Table,
    ) -> Result<arena::Id<ast::Scope>, ParseError> {
        let mut bindings = vec![];

        while self.lexer.try_indent() {
            let _indent = self.consume(TokenKind::Indent);
            let binding = self.parse_binding(scopes, arena)?;
            bindings.push(binding)
        }

        match scopes.new_scope(bindings) {
            Ok(id) => Ok(id),
            Err(error) => Err(ParseError::AstError(error)),
        }
    }

    fn parse_top_level_binding(
        &mut self,
        scopes: &mut ast::Scopes,
        arena: &mut intern::Table,
        identifier: lex::Identifier,
    ) -> Result<ast::Binding, ParseError> {
        let _equal = self.consume(TokenKind::Equal)?;
        let value = match self.parse_value(arena)? {
            Some(value) => value,
            None => return Err(ParseError::InvalidValue),
        };
        let _newline = self.consume(TokenKind::Newline)?;

        let top = scopes.top();
        let bytes = scopes.get_scope(top).evaluate(&value);

        Ok(ast::Binding::new(identifier, bytes))
    }

    fn parse_binding(
        &mut self,
        scopes: &mut ast::Scopes,
        arena: &mut intern::Table,
    ) -> Result<ast::Binding, ParseError> {
        let identifier = self.parse_identifier(arena)?;
        let _equal = self.consume(TokenKind::Equal)?;
        let value = match self.parse_value(arena)? {
            Some(value) => value,
            None => return Err(ParseError::InvalidValue),
        };
        let _newline = self.consume(TokenKind::Newline)?;

        let top = scopes.top();
        let bytes = scopes.get_scope(top).evaluate(&value);

        Ok(ast::Binding::new(identifier, bytes))
    }

    fn parse_identifier(
        &mut self,
        arena: &mut intern::Table,
    ) -> Result<lex::Identifier, ParseError> {
        let token = self.consume(TokenKind::Identifier)?;
        let name = self.lexer.lexeme(token);
        Ok(lex::Identifier::new(arena, name))
    }

    fn parse_value(&mut self, arena: &mut intern::Table) -> Result<Option<ast::Value>, ParseError> {
        match self.lexer.lex_value(arena) {
            Err(error) => Err(ParseError::LexError(error)),
            Ok(None) => Ok(None),
            Ok(Some(value)) => Ok(Some(ast::Value::new(value))),
        }
    }

    fn advance(&mut self) -> Result<Option<Token<TokenKind>>, ParseError> {
        match self.lexer.lex() {
            Ok(token) => Ok(token),
            Err(error) => Err(ParseError::LexError(error)),
        }
    }

    fn advance_decl(&mut self) -> Result<Option<Token<DeclKind>>, ParseError> {
        match self.lexer.lex_decl() {
            Ok(token) => Ok(token),
            Err(error) => Err(ParseError::LexError(error)),
        }
    }

    fn consume(&mut self, expected: TokenKind) -> Result<Token<TokenKind>, ParseError> {
        match self.advance()? {
            None => Err(ParseError::UnexpectedEof),
            Some(token) => {
                let got = token.kind();
                if got == expected {
                    Ok(token)
                } else {
                    Err(ParseError::Expected { expected, got })
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &blob::View) -> Result<ast::File, ParseError> {
        let mut arena = intern::Table::new();
        let mut parser = Parser::new(input);
        parser.parse(&mut arena)
    }

    #[test]
    fn empty() {
        let ninja = b"";
        let file = parse(ninja).expect("failed to parse empty string");
        assert_eq!(file.declarations().count(), 0);
    }

    #[test]
    fn build() {
        for num_outputs in 1..4 {
            for num_implicit_outputs in 0..3 {
                for num_inputs in 0..3 {
                    for num_implicit_inputs in 0..3 {
                        for _num_order_inputs in 0..3 {
                            for num_bindings in 0..3 {
                                let mut ninja = vec![];
                                ninja.extend("build".bytes());
                                for output in 0..num_outputs {
                                    ninja.extend(format!(" output{}", output).bytes());
                                }
                                if num_implicit_outputs > 0 {
                                    ninja.extend(" |".bytes())
                                }
                                for implicit_output in 0..num_implicit_outputs {
                                    ninja.extend(
                                        format!(" implicit_output{}", implicit_output).bytes(),
                                    );
                                }
                                ninja.extend(": rulename".bytes());
                                for input in 0..num_inputs {
                                    ninja.extend(format!(" input{}", input).bytes());
                                }
                                if num_implicit_inputs > 0 {
                                    ninja.extend(" |".bytes())
                                }
                                for implicit_input in 0..num_implicit_inputs {
                                    ninja.extend(
                                        format!(" implicit_input{}", implicit_input).bytes(),
                                    );
                                }
                                if num_implicit_inputs > 0 {
                                    ninja.extend(" ||".bytes())
                                }
                                for order_input in 0..num_implicit_inputs {
                                    ninja.extend(format!(" order_input{}", order_input).bytes());
                                }
                                ninja.extend("\n".bytes());
                                for binding in 0..num_bindings {
                                    ninja.extend(
                                        format!("    var{} = value{}\n", binding, binding).bytes(),
                                    )
                                }

                                let file = parse(&ninja).expect("failed to parse build");
                                assert_eq!(file.declarations().count(), 1);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn invalid_build() {
        let invalid_builds: &[&blob::View] = &[
            b"build output = input\n",
            b"build output : rulename :\n",
            b"build",
        ];
        for ninja in invalid_builds.iter() {
            let result = parse(ninja);
            assert!(result.is_err());
        }
    }

    #[test]
    fn pool() {
        let ninja = b"pool mypool\n    depth = 23\n";
        parse(ninja).expect("failed to parse pool");
    }

    #[test]
    fn pool_variable_pre() {
        let ninja = b"d = 23\npool mypool\n    depth = $d\n";
        parse(ninja).expect("failed to parse pool");
    }

    #[test]
    fn pool_invalid_variable_pre() {
        let ninja = b"d = -23\npool mypool\n    depth = $d\n";
        let err = parse(ninja);
        assert!(err.is_err());
    }

    #[test]
    fn pool_invalid_variable_post() {
        let ninja = b"pool mypool\n    depth = $d\nd = -23\n";
        parse(ninja).expect("failed to parse pool");
    }

    #[test]
    fn build_unset_pool() {
        let ninja = b"build mything : myrule myinput\n    pool =\n";
        parse(ninja).expect("failed to parse pool");
    }
}
