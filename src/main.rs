use logos::Logos;

#[derive(Logos, Debug, PartialEq)]
enum Line
{
    #[token("\n")]
    NewLine,
    #[token(" ")]
    Space,
    #[regex(r"[0-9]+", |lex| lex.slice().parse().ok())]
    Entry(u32),
    #[regex(r"[ \t\f]+", logos::skip)]
    Remainder,
}

fn main()
{
    let input = "
 1  10.64.0.1 (10.64.0.1)  17.249 ms  17.186 ms  17.243 ms
 2  static-68-235-44-1.cust.tzulo.com (68.235.44.1)  17.227 ms  17.213 ms  17.200 ms
 ";

    let mut lex = Line::lexer(input);
    while let Some(x) = lex.next() {
        match x {
            Ok(Line::Entry(x)) => println!("Entry: {}", x),
            Ok(Line::Remainder) => println!("Remainder: {}", lex.slice()),
            Ok(Line::Space) => println!("{}", lex.slice()),
            Ok(Line::NewLine) => println!("NewLine"),
            Err(_) => println!("Other: {}", lex.slice()),
        }
    }

}
