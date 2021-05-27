use crate::lexer::Lexer;
use crate::lexer::LexerError;
use crate::lexer::Loc;
use crate::lexer::ParserLanguage;
use crate::lexer::StrLit;
use crate::lexer::StrLitDecodeError;
use crate::lexer::Token;
use crate::lexer::TokenWithLocation;
use std::fmt;

#[derive(Debug)]
pub enum TokenizerError {
    LexerError(LexerError),
    StrLitDecodeError(StrLitDecodeError),
    InternalError,
    IncorrectInput, // TODO: too broad
    UnexpectedEof,
    ExpectStrLit,
    ExpectIntLit,
    ExpectFloatLit,
    ExpectIdent,
    ExpectNamedIdent(String),
    ExpectChar(char),
    ExpectAnyChar(Vec<char>),
}

impl fmt::Display for TokenizerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenizerError::LexerError(e) => write!(f, "{}", e),
            TokenizerError::StrLitDecodeError(e) => write!(f, "{}", e),
            TokenizerError::InternalError => write!(f, "Internal tokenizer error"),
            TokenizerError::IncorrectInput => write!(f, "Incorrect input"),
            TokenizerError::UnexpectedEof => write!(f, "Unexpected EOF"),
            TokenizerError::ExpectStrLit => write!(f, "Expecting string literal"),
            TokenizerError::ExpectIntLit => write!(f, "Expecting int literal"),
            TokenizerError::ExpectFloatLit => write!(f, "Expecting float literal"),
            TokenizerError::ExpectIdent => write!(f, "Expecting identifier"),
            TokenizerError::ExpectNamedIdent(n) => write!(f, "Expecting identifier {}", n),
            TokenizerError::ExpectChar(c) => write!(f, "Expecting char {}", c),
            TokenizerError::ExpectAnyChar(c) => write!(
                f,
                "Expecting one of: {}",
                c.iter()
                    .map(|c| format!("{}", c))
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        }
    }
}

impl std::error::Error for TokenizerError {}

pub type TokenizerResult<R> = Result<R, TokenizerError>;

impl From<LexerError> for TokenizerError {
    fn from(e: LexerError) -> Self {
        TokenizerError::LexerError(e)
    }
}

impl From<StrLitDecodeError> for TokenizerError {
    fn from(e: StrLitDecodeError) -> Self {
        TokenizerError::StrLitDecodeError(e)
    }
}

#[derive(Clone)]
pub struct Tokenizer<'a> {
    lexer: Lexer<'a>,
    next_token: Option<TokenWithLocation>,
    last_token_loc: Option<Loc>,
}

#[allow(dead_code)]
impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a str, comment_style: ParserLanguage) -> Tokenizer<'a> {
        Tokenizer {
            lexer: Lexer::new(input, comment_style),
            next_token: None,
            last_token_loc: None,
        }
    }

    pub fn loc(&self) -> Loc {
        // After lookahead return the location of the next token
        self.next_token
            .as_ref()
            .map(|t| t.loc)
            // After token consumed return the location of that token
            .or(self.last_token_loc)
            // Otherwise return the position of lexer
            .unwrap_or(self.lexer.loc)
    }

    pub fn lookahead_loc(&mut self) -> Loc {
        drop(self.lookahead());
        // TODO: does not handle EOF properly
        self.loc()
    }

    fn lookahead(&mut self) -> TokenizerResult<Option<&Token>> {
        Ok(match self.next_token {
            Some(ref token) => Some(&token.token),
            None => {
                self.next_token = self.lexer.next_token()?;
                self.last_token_loc = self.next_token.as_ref().map(|t| t.loc);
                match self.next_token {
                    Some(ref token) => Some(&token.token),
                    None => None,
                }
            }
        })
    }

    pub fn lookahead_some(&mut self) -> TokenizerResult<&Token> {
        match self.lookahead()? {
            Some(token) => Ok(token),
            None => Err(TokenizerError::UnexpectedEof),
        }
    }

    /// Can be called only after lookahead, otherwise it's error
    pub fn advance(&mut self) -> TokenizerResult<Token> {
        self.next_token
            .take()
            .map(|TokenWithLocation { token, .. }| token)
            .ok_or(TokenizerError::InternalError)
    }

    /// No more tokens
    pub fn syntax_eof(&mut self) -> TokenizerResult<bool> {
        Ok(self.lookahead()?.is_none())
    }

    pub fn next_token_if_map<P, R>(&mut self, p: P) -> TokenizerResult<Option<R>>
    where
        P: FnOnce(&Token) -> Option<R>,
    {
        self.lookahead()?;
        let v = match self.next_token {
            Some(ref token) => match p(&token.token) {
                Some(v) => v,
                None => return Ok(None),
            },
            _ => return Ok(None),
        };
        self.next_token = None;
        Ok(Some(v))
    }

    pub fn next_token_check_map<P, R, E>(&mut self, p: P) -> Result<R, E>
    where
        P: FnOnce(&Token) -> Result<R, E>,
        E: From<TokenizerError>,
    {
        self.lookahead()?;
        let r = match self.next_token {
            Some(ref token) => p(&token.token)?,
            None => return Err(TokenizerError::UnexpectedEof.into()),
        };
        self.next_token = None;
        Ok(r)
    }

    fn next_token_if<P>(&mut self, p: P) -> TokenizerResult<Option<Token>>
    where
        P: FnOnce(&Token) -> bool,
    {
        self.next_token_if_map(|token| if p(token) { Some(token.clone()) } else { None })
    }

    pub fn next_ident_if_in(&mut self, idents: &[&str]) -> TokenizerResult<Option<String>> {
        let v = match self.lookahead()? {
            Some(&Token::Ident(ref next)) => {
                if idents.iter().any(|&i| i == next) {
                    next.clone()
                } else {
                    return Ok(None);
                }
            }
            _ => return Ok(None),
        };
        self.advance()?;
        Ok(Some(v))
    }

    pub fn next_ident_if_eq(&mut self, word: &str) -> TokenizerResult<bool> {
        Ok(self.next_ident_if_in(&[word])? != None)
    }

    pub fn next_ident_expect_eq(&mut self, word: &str) -> TokenizerResult<()> {
        if self.next_ident_if_eq(word)? {
            Ok(())
        } else {
            Err(TokenizerError::ExpectNamedIdent(word.to_owned()))
        }
    }

    pub fn next_ident_if_eq_error(&mut self, word: &str) -> TokenizerResult<()> {
        if self.clone().next_ident_if_eq(word)? {
            return Err(TokenizerError::IncorrectInput);
        }
        Ok(())
    }

    pub fn next_symbol_if_eq(&mut self, symbol: char) -> TokenizerResult<bool> {
        Ok(self.next_token_if(|token| matches!(token, &Token::Symbol(c) if c == symbol))? != None)
    }

    pub fn next_symbol_expect_eq(&mut self, symbol: char) -> TokenizerResult<()> {
        if self.lookahead_is_symbol(symbol)? {
            self.advance()?;
            Ok(())
        } else {
            Err(TokenizerError::ExpectChar(symbol))
        }
    }

    pub fn lookahead_if_symbol(&mut self) -> TokenizerResult<Option<char>> {
        Ok(match self.lookahead()? {
            Some(&Token::Symbol(c)) => Some(c),
            _ => None,
        })
    }

    pub fn lookahead_is_symbol(&mut self, symbol: char) -> TokenizerResult<bool> {
        Ok(self.lookahead_if_symbol()? == Some(symbol))
    }

    pub fn lookahead_is_ident(&mut self, ident: &str) -> TokenizerResult<bool> {
        Ok(match self.lookahead()? {
            Some(Token::Ident(i)) => i == ident,
            _ => false,
        })
    }

    pub fn next_ident(&mut self) -> TokenizerResult<String> {
        self.next_token_check_map(|token| match *token {
            Token::Ident(ref ident) => Ok(ident.clone()),
            _ => Err(TokenizerError::ExpectIdent),
        })
    }

    pub fn next_str_lit(&mut self) -> TokenizerResult<StrLit> {
        self.next_token_check_map(|token| match *token {
            Token::StrLit(ref str_lit) => Ok(str_lit.clone()),
            _ => Err(TokenizerError::ExpectStrLit),
        })
    }
}

#[cfg(test)]
mod test {

    use super::*;

    fn tokenize<P, R>(input: &str, what: P) -> R
    where
        P: FnOnce(&mut Tokenizer) -> TokenizerResult<R>,
    {
        let mut tokenizer = Tokenizer::new(input, ParserLanguage::Proto);
        let r = what(&mut tokenizer).expect(&format!("parse failed at {}", tokenizer.loc()));
        let eof = tokenizer
            .syntax_eof()
            .expect(&format!("check eof failed at {}", tokenizer.loc()));
        assert!(eof, "{}", tokenizer.loc());
        r
    }

    #[test]
    fn test_ident() {
        let msg = r#"  aabb_c  "#;
        let mess = tokenize(msg, |p| p.next_ident().map(|s| s.to_owned()));
        assert_eq!("aabb_c", mess);
    }

    #[test]
    fn test_str_lit() {
        let msg = r#"  "a\nb"  "#;
        let mess = tokenize(msg, |p| p.next_str_lit());
        assert_eq!(
            StrLit {
                escaped: r#"a\nb"#.to_owned()
            },
            mess
        );
    }
}
