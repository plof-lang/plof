use std::rc::Rc;

use super::*;
use super::ParserError;

use super::lexer::TokenType;

pub struct Parser {
    traveler: Traveler,
}

#[allow(dead_code)]
impl Parser {
    pub fn new(traveler: Traveler) -> Parser {
        Parser {
            traveler,
        }
    }

    pub fn parse(&mut self) -> ParserResult<Vec<Statement>> {
        let mut stack = Vec::new();
        while self.traveler.remaining() > 2 {
            stack.push(try!(self.statement()));
            self.traveler.next();
        }

        Ok(stack)
    }

    pub fn skip_white(&mut self) -> ParserResult<()> {
        while self.traveler.current_content() == "\n" || self.traveler.current().token_type == TokenType::EOL {
            self.traveler.next();
        }
        
        Ok(())
    }

    pub fn statement(&mut self) -> ParserResult<Statement> {
        match self.traveler.current().token_type {
            TokenType::EOL => {
                self.traveler.next();
                self.statement()
            },
            TokenType::Keyword => match self.traveler.current_content().as_str() {
                "return" => {
                    self.traveler.next();

                    if self.traveler.current_content() == "\n" {
                        Ok(Statement::Return(None))
                    } else {
                        Ok(Statement::Return(Some(Rc::new(try!(self.expression())))))
                    }
                }
                _ => Err(ParserError::new_pos(self.traveler.current().position, &format!("unexpected: {}", self.traveler.current_content()))),
            },
            _ => Ok(Statement::Expression(Rc::new(try!(self.expression())))),
        }
    }

    pub fn term(&mut self) -> ParserResult<Expression> {
        match self.traveler.current().token_type {
            TokenType::EOL => {
                self.traveler.next();
                match self.traveler.current().token_type {
                    TokenType::Block(_) => return Ok(Expression::Block(Rc::new(try!(self.block())))),
                    TokenType::EOL      => return Ok(Expression::EOF),
                    _ => (),
                }
            },
            _ => (),
        }

        match self.traveler.current().token_type {
            TokenType::IntLiteral    => Ok(Expression::NumberLiteral(self.traveler.current_content().parse::<f64>().unwrap())),
            TokenType::FloatLiteral  => Ok(Expression::NumberLiteral(self.traveler.current_content().parse::<f64>().unwrap())),
            TokenType::BoolLiteral   => Ok(Expression::BoolLiteral(self.traveler.current_content() == "true")),
            TokenType::StringLiteral => Ok(Expression::StringLiteral(Rc::new(self.traveler.current_content().clone()))),
            TokenType::Identifier    => {
                let id = Expression::Identifier(Rc::new(self.traveler.current_content()));
                let name = Rc::new(self.traveler.current_content());

                self.traveler.next();

                match self.traveler.current().token_type {
                    TokenType::IntLiteral |
                    TokenType::FloatLiteral |
                    TokenType::BoolLiteral |
                    TokenType::StringLiteral |
                    TokenType::Identifier |
                    TokenType::Symbol => {
                        if self.traveler.current().token_type == TokenType::Symbol {
                            match self.traveler.current_content().as_str() {
                                "(" | ")" => (),
                                "!"       => return Ok(Expression::Call(Rc::new(id), Rc::new(vec!()))),
                                "="       => {
                                    self.traveler.next();
                                    let expr = self.expression()?;
                                    self.traveler.next();

                                    return Ok(Expression::Definition(None, name, Rc::new(expr)))
                                },
                                ":"       => {
                                    self.traveler.next();
                                    let expr = self.expression()?;
                                    println!("found in key: {}", self.traveler.current_content());

                                    self.traveler.next();

                                    return Ok(Expression::Key(None, name, Rc::new(expr)))
                                }
                                _   => return Err(ParserError::new_pos(self.traveler.current().position, &format!("unexpected: {}", self.traveler.current_content()))),
                            }
                        }

                        let call = try!(self.call(id));

                        self.traveler.next();

                        return Ok(call)
                    },

                    _ => (),
                }

                self.traveler.prev();

                Ok(id)
            },

            TokenType::Symbol => match self.traveler.current_content().as_str() {
                "[" => {
                    self.traveler.next();
                    
                    let mut body = Vec::new();
                    
                    while self.traveler.current_content() != "]" {
                        body.push(self.expression()?);
                        self.traveler.next();
                        
                        if self.traveler.current_content() == "," {
                            self.traveler.next();
                        }
                        
                        self.skip_white()?;
                    }
                    
                    self.traveler.next(); // "]"
                    
                    Ok(Expression::DictLiteral(Rc::new(body)))
                },

                "(" => {
                    self.traveler.next();
                    let expr = self.expression()?;
                    self.traveler.next();
                    self.skip_white()?;
                    self.traveler.expect_content(")")?;

                    self.traveler.next();

                    match self.traveler.current().token_type {
                        TokenType::IntLiteral |
                        TokenType::FloatLiteral |
                        TokenType::BoolLiteral |
                        TokenType::StringLiteral |
                        TokenType::Identifier |
                        TokenType::Symbol => {
                            if self.traveler.current().token_type == TokenType::Symbol {
                                match self.traveler.current_content().as_str() {
                                    "(" => (),
                                    _   => return Err(ParserError::new_pos(self.traveler.current().position, &format!("unexpected here: {}", self.traveler.current_content()))),
                                }
                            }

                            let call = try!(self.call(expr));

                            self.traveler.next();

                            return Ok(call)
                        },

                        _ => (),
                    }

                    self.traveler.prev();

                    Ok(expr)
                },

                "," | ")" => { // bad hack here
                    self.traveler.next();
                    return self.term()
                },

                _ => Err(ParserError::new_pos(self.traveler.current().position, &format!("unexpected symbol: {}", self.traveler.current_content()))),
            },

            TokenType::Type => {
                let retty = get_type(&self.traveler.current_content()).unwrap();

                self.traveler.next();

                match self.traveler.current().token_type {
                    TokenType::Identifier => {
                        let id = self.traveler.current_content();

                        self.traveler.next();

                        if self.traveler.current_content() == "=" {
                            self.traveler.next();
                            
                            let expr = self.expression()?;
                            self.traveler.next();

                            Ok(Expression::Definition(Some(retty), Rc::new(id), Rc::new(expr)))
                        } else if self.traveler.current_content() == ":" {
                            self.traveler.next();
                            
                            let expr = self.expression()?;
                            self.traveler.next();

                            Ok(Expression::Key(Some(retty), Rc::new(id), Rc::new(expr)))
                        } else {
                            Err(ParserError::new_pos(self.traveler.current().position, &format!("expected '=' or ':', found: {}", self.traveler.current_content())))
                        }
                    },

                    TokenType::Symbol => match self.traveler.current_content().as_str() {
                        "(" => {
                            self.traveler.next();

                            let mut param_names = Vec::new();
                            let mut param_types = Vec::new();
                            
                            while self.traveler.current_content() != ")" {
                                let mut t: Type = Type::Any;
                                
                                match self.traveler.current().token_type {
                                    TokenType::Type => {
                                        t = get_type(&self.traveler.current_content()).unwrap();
                                        self.traveler.next();
                                    },

                                    TokenType::Identifier => (),
                                    
                                    TokenType::Symbol => match self.traveler.current_content().as_str() {
                                        "," | "..." => (),
                                        _ => return Err(ParserError::new_pos(self.traveler.current().position, &format!("unexpected: {}", self.traveler.current_content()))),
                                    },

                                    _ => return Err(ParserError::new_pos(self.traveler.current().position, &format!("unexpected: {}", self.traveler.current_content()))),
                                }

                                if self.traveler.current_content() == "," {
                                    self.traveler.next();
                                } else {
                                    if self.traveler.current_content() == "..." {
                                        t = Type::Many(Rc::new(t));
                                        self.traveler.next();
                                    } else {
                                        let id = Rc::new(try!(self.traveler.expect(TokenType::Identifier)));
                                        self.traveler.next();

                                        param_names.push(id);
                                    }
                                    
                                    param_types.push(t);
                                }
                            }

                            self.traveler.next(); // skips closing ')'

                            let mut name = None;

                            if self.traveler.current().token_type == TokenType::Identifier {
                                name = Some(Rc::new(self.traveler.current_content()));
                                self.traveler.next();
                            }

                            try!(self.traveler.expect_content("="));

                            self.traveler.next();

                            let body: Rc<Vec<Statement>>;

                            match self.traveler.current_content().as_str() {
                                "\n" => {
                                    self.traveler.next();
                                    body = Rc::new(try!(self.block()));
                                },
                                _ => body = Rc::new(vec![try!(self.statement())]),
                            }

                            Ok(Expression::Lambda {
                                name,
                                retty,
                                param_names,
                                param_types,
                                body,
                            })
                        },

                        _ => Err(ParserError::new_pos(self.traveler.current().position, &format!("unexpected: {}", self.traveler.current_content()))),
                    },

                    _ => Err(ParserError::new_pos(self.traveler.current().position, &format!("unexpected: {}", self.traveler.current_content()))),
                }
            },

            _ => Err(ParserError::new_pos(self.traveler.current().position, &format!("unexpected: {:#?}", self.traveler.current()))),
        }
    }

    fn block(&mut self) -> ParserResult<Vec<Statement>> {
        match self.traveler.current().token_type {
            TokenType::Block(ref v) => {
                let mut p = Parser::new(Traveler::new(v.clone()));
                Ok(try!(p.parse()))
            },
            _ => Err(ParserError::new_pos(self.traveler.current().position, &format!("expected block, found: {}", self.traveler.current_content()))),
        }
    }

    fn expression(&mut self) -> ParserResult<Expression> {
        if self.traveler.current_content() == "\n" {
            self.traveler.next();
        }

        let expr = try!(self.term());

        self.traveler.next();
        if self.traveler.remaining() > 0 {
            if self.traveler.current().token_type == TokenType::Operator {
                return self.operation(expr)
            }
            self.traveler.prev();
        }
        Ok(expr)
    }

    fn call(&mut self, caller: Expression) -> ParserResult<Expression> {
        let mut args = Vec::new();

        while self.traveler.current_content() != ")" && self.traveler.current_content() != "\n" {
            args.push(try!(self.expression()));
            self.traveler.next();

            if self.traveler.current_content() == "," {
                self.traveler.next();
            }
        }

        Ok(Expression::Call(Rc::new(caller), Rc::new(args)))
    }

    fn operation(&mut self, expression: Expression) -> ParserResult<Expression> {

        let mut ex_stack = vec![expression];
        let mut op_stack: Vec<(Operand, u8)> = Vec::new();

        op_stack.push(get_operand(&self.traveler.current_content()).unwrap());
        self.traveler.next();

        if self.traveler.current_content() == "\n" {
            self.traveler.next();
        }

        ex_stack.push(try!(self.term()));

        let mut done = false;
        while ex_stack.len() > 1 {
            if !done && self.traveler.next() {
                if self.traveler.current().token_type != TokenType::Operator {
                    self.traveler.prev();
                    done = true;
                    continue
                }

                let (op, precedence) = get_operand(&self.traveler.current_content()).unwrap();

                if precedence >= op_stack.last().unwrap().1 {
                    let left  = ex_stack.pop().unwrap();
                    let right = ex_stack.pop().unwrap();

                    ex_stack.push(Expression::Operation {
                        right: Rc::new(left),
                        op:    op_stack.pop().unwrap().0,
                        left:  Rc::new(right)
                    });

                    self.traveler.next();

                    ex_stack.push(try!(self.term()));
                    op_stack.push((op, precedence));

                    continue
                }

                self.traveler.next();

                ex_stack.push(try!(self.term()));
                op_stack.push((op, precedence));
            }

            let left  = ex_stack.pop().unwrap();
            let right = ex_stack.pop().unwrap();

            ex_stack.push(Expression::Operation {
                right: Rc::new(left),
                op:    op_stack.pop().unwrap().0,
                left:  Rc::new(right)
            });
        }

        Ok(ex_stack.pop().unwrap())
    }
}
