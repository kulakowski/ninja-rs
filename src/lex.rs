use crate::blob;
use crate::intern;

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub struct Identifier {
    id: intern::Symbol,
}

impl Identifier {
    pub fn new(arena: &mut intern::Table, name: &blob::View) -> Identifier {
        let id = arena.insert(name);
        Identifier { id }
    }
}

pub struct Value {
    pub parts: Vec<ValuePart>,
}

impl Value {
    fn new(parts: Vec<ValuePart>) -> Option<Value> {
        Some(Value { parts })
    }
}

pub enum ValuePart {
    Text(blob::Blob),
    Variable(Identifier),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenKind {
    Pipe,
    PipePipe,
    Equal,
    Colon,

    Identifier,

    Newline,
    Indent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclKind {
    Default,
    Rule,
    Build,
    Pool,
    Include,
    Subninja,

    Identifier,

    Newline,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceLocation {
    range: (usize, usize),
    line: usize,
}

impl SourceLocation {
    fn range(&self) -> std::ops::Range<usize> {
        self.range.0..self.range.1
    }

    pub fn line(&self) -> usize {
        self.line
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token<Kind> {
    kind: Kind,
    location: SourceLocation,
}

impl<Kind: Copy> Token<Kind> {
    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn location(&self) -> SourceLocation {
        self.location
    }
}

enum Lexed {
    Token(Token<TokenKind>),
    Eof,
    Comment,
}

#[derive(Debug)]
pub enum LexError {
    UnknownToken,
    InvalidDeclStart,
}

pub struct Lexer<'input> {
    input: &'input blob::View,
    current: std::ops::Range<usize>,
    line: usize,
}

impl<'input> Lexer<'input> {
    pub fn new(input: &'input blob::View) -> Lexer<'input> {
        Lexer {
            input,
            current: 0..0,
            line: 1,
        }
    }

    pub fn lexeme<Kind>(&self, token: Token<Kind>) -> &'input blob::View {
        &self.input[token.location.range()]
    }

    pub fn try_indent(&mut self) -> bool {
        matches!(self.peek(), Some(b' '))
    }

    fn lex_dollar(&mut self, arena: &mut intern::Table) -> Result<ValuePart, LexError> {
        match self.peek() {
            None => Err(LexError::UnknownToken),
            Some(b) => match b {
                b' ' | b':' | b'$' => {
                    self.advance();
                    Ok(ValuePart::Text(blob::Blob::new(&[b])))
                }
                b'\n' => {
                    self.advance();
                    self.skip_whitespace()?;
                    Ok(ValuePart::Text(blob::Blob::new(b"")))
                }
                b'\r' => {
                    self.advance();
                    match self.peek() {
                        Some(b'\n') => {
                            self.advance();
                            self.skip_whitespace()?;
                            Ok(ValuePart::Text(blob::Blob::new(b"")))
                        }
                        _ => Err(LexError::UnknownToken),
                    }
                }
                b'{' => {
                    self.advance();
                    let mut variable = vec![];
                    loop {
                        match self.peek() {
                            Some(b) if is_identifier(b) => {
                                self.advance();
                                variable.push(b)
                            }
                            Some(b'}') => {
                                self.advance();
                                break;
                            }
                            _ => return Err(LexError::UnknownToken),
                        }
                    }
                    let variable = Identifier::new(arena, &variable);
                    Ok(ValuePart::Variable(variable))
                }
                b if is_bare_identifier(b) => {
                    self.advance();
                    let mut variable = blob::Builder::new();
                    variable.push(b);
                    loop {
                        match self.peek() {
                            Some(b) if is_bare_identifier(b) => {
                                self.advance();
                                variable.push(b)
                            }
                            _ => break,
                        }
                    }
                    let variable = variable.blob();
                    let variable = Identifier::new(arena, &variable);
                    Ok(ValuePart::Variable(variable))
                }
                _ => Err(LexError::UnknownToken),
            },
        }
    }

    pub fn lex_value(&mut self, arena: &mut intern::Table) -> Result<Option<Value>, LexError> {
        let mut parts = vec![];
        loop {
            match self.peek() {
                None => return Err(LexError::UnknownToken),
                Some(b) => match b {
                    b'\n' => break,

                    b'$' => {
                        self.advance();
                        let part = self.lex_dollar(arena)?;
                        parts.push(part)
                    }

                    b => {
                        self.advance();
                        parts.push(ValuePart::Text(blob::Blob::new(&[b])))
                    }
                },
            }
        }

        self.start_next_token();

        Ok(Value::new(parts))
    }

    pub fn lex_target(&mut self, arena: &mut intern::Table) -> Result<Option<Value>, LexError> {
        let mut parts = vec![];
        loop {
            match self.peek() {
                None => return Err(LexError::UnknownToken),
                Some(b) => match b {
                    b'|' | b':' | b' ' | b'\n' => break,

                    b'$' => {
                        self.advance();
                        let part = self.lex_dollar(arena)?;
                        parts.push(part)
                    }

                    b => {
                        self.advance();
                        parts.push(ValuePart::Text(blob::Blob::new(&[b])))
                    }
                },
            }
        }
        self.skip_whitespace()?;
        self.start_next_token();
        if parts.is_empty() {
            Ok(None)
        } else {
            Ok(Value::new(parts))
        }
    }

    pub fn lex(&mut self) -> Result<Option<Token<TokenKind>>, LexError> {
        loop {
            let lexed = self.lex_one()?;
            match lexed {
                Lexed::Token(token) => {
                    if token.kind() != TokenKind::Newline {
                        self.skip_whitespace()?
                    }
                    return Ok(Some(token));
                }
                Lexed::Eof => return Ok(None),
                Lexed::Comment => (),
            }
        }
    }

    pub fn lex_decl(&mut self) -> Result<Option<Token<DeclKind>>, LexError> {
        match self.lex()? {
            Some(token) => match self.decl(token) {
                Some(decl) => Ok(Some(decl)),
                None => Err(LexError::InvalidDeclStart),
            },
            None => Ok(None),
        }
    }

    fn lex_one(&mut self) -> Result<Lexed, LexError> {
        match self.peek() {
            None => Ok(Lexed::Eof),
            Some(b) => match b {
                b'=' => {
                    self.advance();
                    Ok(self.token(TokenKind::Equal))
                }
                b':' => {
                    self.advance();
                    Ok(self.token(TokenKind::Colon))
                }
                b'|' => {
                    self.advance();
                    match self.peek() {
                        Some(b'|') => {
                            self.advance();
                            Ok(self.token(TokenKind::PipePipe))
                        }
                        _ => Ok(self.token(TokenKind::Pipe)),
                    }
                }

                b if is_identifier(b) => {
                    self.advance();

                    loop {
                        match self.peek() {
                            Some(b) if is_identifier(b) => self.advance(),
                            _ => break,
                        }
                    }

                    Ok(self.token(TokenKind::Identifier))
                }

                b' ' => {
                    self.advance();

                    while let Some(b' ') = self.peek() {
                        self.advance()
                    }

                    Ok(self.token(TokenKind::Indent))
                }

                b'\r' => {
                    self.advance();
                    match self.peek() {
                        Some(b'\n') => {
                            self.advance();
                            Ok(self.token(TokenKind::Newline))
                        }
                        _ => self.error(LexError::UnknownToken),
                    }
                }

                b'\n' => {
                    self.advance();
                    Ok(self.token(TokenKind::Newline))
                }

                b'#' => {
                    self.advance();

                    loop {
                        match self.peek() {
                            Some(b'\n') => {
                                self.line += 1;
                                self.advance();
                                self.start_next_token();
                                break;
                            }
                            None => return self.error(LexError::UnknownToken),
                            _ => self.advance(),
                        }
                    }

                    Ok(Lexed::Comment)
                }

                _ => self.error(LexError::UnknownToken),
            },
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.current.end).cloned()
    }

    fn advance(&mut self) {
        self.current.end += 1
    }

    fn token(&mut self, kind: TokenKind) -> Lexed {
        let range = (self.current.start, self.current.end);
        let line = self.line;
        if kind == TokenKind::Newline {
            self.line += 1
        }
        let location = SourceLocation { range, line };
        self.start_next_token();
        Lexed::Token(Token { kind, location })
    }

    fn error<T>(&self, error: LexError) -> Result<T, LexError> {
        Err(error)
    }

    fn decl(&mut self, token: Token<TokenKind>) -> Option<Token<DeclKind>> {
        let kind = match token.kind() {
            TokenKind::Newline => DeclKind::Newline,
            TokenKind::Identifier => self.keyword(token),
            _ => return None,
        };
        let location = token.location();
        Some(Token { kind, location })
    }

    fn keyword(&self, token: Token<TokenKind>) -> DeclKind {
        const KEYWORDS: [(&[u8], DeclKind); 6] = [
            (b"default", DeclKind::Default),
            (b"rule", DeclKind::Rule),
            (b"build", DeclKind::Build),
            (b"pool", DeclKind::Pool),
            (b"include", DeclKind::Include),
            (b"subninja", DeclKind::Subninja),
        ];

        let lexeme = self.lexeme(token);

        for (keyword, kind) in KEYWORDS.iter() {
            if lexeme == *keyword {
                return *kind;
            }
        }

        DeclKind::Identifier
    }

    fn start_next_token(&mut self) {
        self.current.start = self.current.end
    }

    fn skip_whitespace(&mut self) -> Result<(), LexError> {
        loop {
            match self.peek() {
                Some(b' ') => self.advance(),
                Some(b'$') => {
                    let dollar = self.current.end;
                    self.advance();
                    match self.peek() {
                        Some(b'\n') => self.advance(),
                        Some(b' ') => self.advance(),
                        Some(b'\r') => {
                            self.advance();
                            match self.peek() {
                                Some(b'\n') => self.advance(),
                                _ => {
                                    self.current.end = dollar;
                                    break;
                                }
                            }
                        }
                        _ => {
                            self.current.end = dollar;
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
        self.start_next_token();
        Ok(())
    }
}

fn is_bare_identifier(b: u8) -> bool {
    matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-')
}

fn is_identifier(b: u8) -> bool {
    matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-' | b'.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex_all(lexer: &mut Lexer) -> Result<Vec<Token<TokenKind>>, LexError> {
        let mut tokens = vec![];
        while let Some(token) = lexer.lex()? {
            tokens.push(token)
        }
        Ok(tokens)
    }

    #[test]
    fn construction() {
        let _lexer = Lexer::new(b"");
    }

    #[test]
    fn empty() {
        let mut lexer = Lexer::new(b"");
        let tokens = lex_all(&mut lexer).expect("failed to lex empty string");
        assert_eq!(tokens, vec![]);
    }

    #[test]
    fn bare_dollar_sign() {
        let mut lexer = Lexer::new(b"$");
        match lexer.lex() {
            Err(LexError::UnknownToken) => (),
            _ => panic!("incorrectly lexed a bare dollar sign"),
        }
    }

    #[test]
    fn cr_without_newline() {
        let mut lexer = Lexer::new(b"\r");
        match lex_all(&mut lexer) {
            Err(LexError::UnknownToken) => (),
            _ => panic!("incorrectly lexed a carriage return"),
        }
    }

    #[test]
    fn unknown_characters() {
        for unknown in [
            b"~", b"`", b"!", b"@", b"%", b"^", b"&", b"*", b"(", b")", b"[", b"]", b"{", b"}",
            b"'", b"\"", b",", b"<", b">", b"/", b"?", b"+", b"\\", b";",
        ]
        .iter()
        {
            let mut lexer = Lexer::new(*unknown);
            match lexer.lex() {
                Err(LexError::UnknownToken) => (),
                _ => panic!("incorrectly lexed an invalid character"),
            }
        }
    }

    #[test]
    fn one_line() {
        let mut lexer = Lexer::new(b"builddir = b\n");
        let tokens = lex_all(&mut lexer).expect("failed to lex");
        assert_eq!(
            tokens,
            vec![
                Token {
                    kind: TokenKind::Identifier,
                    location: SourceLocation {
                        range: (0, 8),
                        line: 1
                    },
                },
                Token {
                    kind: TokenKind::Equal,
                    location: SourceLocation {
                        range: (9, 10),
                        line: 1
                    },
                },
                Token {
                    kind: TokenKind::Identifier,
                    location: SourceLocation {
                        range: (11, 12),
                        line: 1
                    },
                },
                Token {
                    kind: TokenKind::Newline,
                    location: SourceLocation {
                        range: (12, 13),
                        line: 1
                    },
                },
            ]
        );
    }

    #[test]
    fn escaped_newline() {
        let mut lexer = Lexer::new(b"builddir = $\n    b\n");
        let tokens = lex_all(&mut lexer).expect("failed to lex");
        assert_eq!(
            tokens,
            vec![
                Token {
                    kind: TokenKind::Identifier,
                    location: SourceLocation {
                        range: (0, 8),
                        line: 1
                    },
                },
                Token {
                    kind: TokenKind::Equal,
                    location: SourceLocation {
                        range: (9, 10),
                        line: 1
                    },
                },
                Token {
                    kind: TokenKind::Identifier,
                    location: SourceLocation {
                        range: (17, 18),
                        line: 1
                    },
                },
                Token {
                    kind: TokenKind::Newline,
                    location: SourceLocation {
                        range: (18, 19),
                        line: 1
                    },
                },
            ]
        );
    }

    #[test]
    fn escaped_cr_newline() {
        let mut lexer = Lexer::new(b"builddir = $\r\nb\n");
        let tokens = lex_all(&mut lexer).expect("failed to lex");
        assert_eq!(
            tokens,
            vec![
                Token {
                    kind: TokenKind::Identifier,
                    location: SourceLocation {
                        range: (0, 8),
                        line: 1
                    },
                },
                Token {
                    kind: TokenKind::Equal,
                    location: SourceLocation {
                        range: (9, 10),
                        line: 1
                    },
                },
                Token {
                    kind: TokenKind::Identifier,
                    location: SourceLocation {
                        range: (14, 15),
                        line: 1
                    },
                },
                Token {
                    kind: TokenKind::Newline,
                    location: SourceLocation {
                        range: (15, 16),
                        line: 1
                    },
                },
            ]
        );
    }

    #[test]
    fn multiple_lines() {
        let mut lexer = Lexer::new(b"a\na\na\na\n");
        let tokens = lex_all(&mut lexer).expect("failed to lex");
        assert_eq!(
            tokens,
            vec![
                Token {
                    kind: TokenKind::Identifier,
                    location: SourceLocation {
                        range: (0, 1),
                        line: 1
                    },
                },
                Token {
                    kind: TokenKind::Newline,
                    location: SourceLocation {
                        range: (1, 2),
                        line: 1
                    },
                },
                Token {
                    kind: TokenKind::Identifier,
                    location: SourceLocation {
                        range: (2, 3),
                        line: 2
                    },
                },
                Token {
                    kind: TokenKind::Newline,
                    location: SourceLocation {
                        range: (3, 4),
                        line: 2
                    },
                },
                Token {
                    kind: TokenKind::Identifier,
                    location: SourceLocation {
                        range: (4, 5),
                        line: 3
                    },
                },
                Token {
                    kind: TokenKind::Newline,
                    location: SourceLocation {
                        range: (5, 6),
                        line: 3
                    },
                },
                Token {
                    kind: TokenKind::Identifier,
                    location: SourceLocation {
                        range: (6, 7),
                        line: 4
                    },
                },
                Token {
                    kind: TokenKind::Newline,
                    location: SourceLocation {
                        range: (7, 8),
                        line: 4
                    },
                },
            ]
        );
    }

    #[test]
    fn one_line_value() {
        let mut arena = intern::Table::new();
        let mut lexer = Lexer::new(b"builddir = $BUILDDIR\n");

        let identifier = lexer
            .lex_decl()
            .expect("failed to lex identifier")
            .expect("failed to lex identifier");
        assert_eq!(identifier.kind(), DeclKind::Identifier);

        let equal = lexer
            .lex()
            .expect("failed to lex equal")
            .expect("failed to lex equal");
        assert_eq!(equal.kind(), TokenKind::Equal);

        let _value = lexer
            .lex_value(&mut arena)
            .expect("failed to lex value")
            .expect("failed to lex value");

        let equal = lexer
            .lex()
            .expect("failed to lex newline")
            .expect("failed to lex newline");
        assert_eq!(equal.kind(), TokenKind::Newline);

        let eof = lexer.lex().expect("failed to lex EOF");
        assert!(eof.is_none());
    }

    #[test]
    fn build() {
        let mut arena = intern::Table::new();

        let mut lexer = Lexer::new(b"build output1 output2 | implicit_output1 implicit_output2: rulename input1 input2 | implicit_input1 implicit_input2 || order_input1 order_input2\n");

        let build = lexer
            .lex_decl()
            .expect("failed to lex build")
            .expect("failed to lex build");
        assert_eq!(build.kind(), DeclKind::Build);

        let _output1 = lexer
            .lex_target(&mut arena)
            .expect("failed to lex output1")
            .expect("failed to lex output1");
        let _output2 = lexer
            .lex_target(&mut arena)
            .expect("failed to lex output2")
            .expect("failed to lex output2");
        let nothing = lexer.lex_target(&mut arena).expect("failed to lex nothing");
        assert!(nothing.is_none());

        let pipe = lexer
            .lex()
            .expect("failed to lex pipe")
            .expect("failed to lex pipe");
        assert_eq!(pipe.kind(), TokenKind::Pipe);

        let _implicit_output1 = lexer
            .lex_target(&mut arena)
            .expect("failed to lex implicit_output1")
            .expect("failed to lex implicit_output1");
        let _implicit_output2 = lexer
            .lex_target(&mut arena)
            .expect("failed to lex implicit_output2")
            .expect("failed to lex implicit_output2");
        let nothing = lexer.lex_target(&mut arena).expect("failed to lex nothing");
        assert!(nothing.is_none());

        let colon = lexer
            .lex()
            .expect("failed to lex colon")
            .expect("failed to lex colon");
        assert_eq!(colon.kind(), TokenKind::Colon);

        let rulename = lexer
            .lex()
            .expect("failed to lex rulename")
            .expect("failed to lex rulename");
        assert_eq!(rulename.kind(), TokenKind::Identifier);

        let _input1 = lexer
            .lex_target(&mut arena)
            .expect("failed to lex input1")
            .expect("failed to lex input1");
        let _input2 = lexer
            .lex_target(&mut arena)
            .expect("failed to lex input2")
            .expect("failed to lex input2");
        let nothing = lexer.lex_target(&mut arena).expect("failed to lex nothing");
        assert!(nothing.is_none());

        let pipe = lexer
            .lex()
            .expect("failed to lex pipe")
            .expect("failed to lex pipe");
        assert_eq!(pipe.kind(), TokenKind::Pipe);

        let _implicit_input1 = lexer
            .lex_target(&mut arena)
            .expect("failed to lex implicit_input1")
            .expect("failed to lex implicit_input1");
        let _implicit_input2 = lexer
            .lex_target(&mut arena)
            .expect("failed to lex implicit_input2")
            .expect("failed to lex implicit_input2");
        let nothing = lexer.lex_target(&mut arena).expect("failed to lex nothing");
        assert!(nothing.is_none());

        let pipe_pipe = lexer
            .lex()
            .expect("failed to lex pipe_pipe")
            .expect("failed to lex pipe_pipe");
        assert_eq!(pipe_pipe.kind(), TokenKind::PipePipe);

        let _order_input1 = lexer
            .lex_target(&mut arena)
            .expect("failed to lex order_input1")
            .expect("failed to lex order_input1");
        let _order_input2 = lexer
            .lex_target(&mut arena)
            .expect("failed to lex order_input2")
            .expect("failed to lex order_input2");
        let nothing = lexer.lex_target(&mut arena).expect("failed to lex nothing");
        assert!(nothing.is_none());

        let newline = lexer
            .lex()
            .expect("failed to lex newline")
            .expect("failed to lex newline");
        assert_eq!(newline.kind(), TokenKind::Newline);
    }

    #[test]
    fn value() {
        let mut arena = intern::Table::new();
        let inputs: &[&blob::View] = &[b"${my_variable}\n", b"$my_variable\n"];
        for input in inputs.iter() {
            let mut lexer = Lexer::new(input);
            let value = lexer
                .lex_value(&mut arena)
                .expect("failed to lex value")
                .expect("failed to lex value");
            assert_eq!(value.parts.len(), 1);
        }
    }

    #[test]
    fn target() {
        let mut arena = intern::Table::new();
        let inputs: &[&blob::View] = &[b"${my_variable}\n", b"$my_variable\n"];
        for input in inputs.iter() {
            let mut lexer = Lexer::new(input);
            let value = lexer
                .lex_target(&mut arena)
                .expect("failed to lex target")
                .expect("failed to lex target");
            assert_eq!(value.parts.len(), 1);
        }
    }
}
