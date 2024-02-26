use available_macros::*;

#[available]
fn main() {
    println!("Hello, world!");

    #[cfg(test)]
    let foo = 32;

    #[available(removed = 16)]
    let eep = Eep::Foo;

    #[available(added = 16, removed = 18)]
    let eep = Eep::Bar;

    #[available(added = 18)]
    let eep = make_eep();

    println!("eep: {}", eep_str(&eep));
}

#[available(added = 17)]
fn make_eep() -> Eep {
    return Eep::Baz;
}

#[available]
enum Eep {
    Foo,
    #[available(added = 16, removed = 18)]
    Bar,
    Baz,
}

#[available]
fn eep_str(eep: &Eep) -> &'static str {
    match eep {
        Eep::Foo => "foo",
        #[available(added = 16, removed = 18)]
        Eep::Bar => "bar",
        Eep::Baz => "baz",
    }
}
