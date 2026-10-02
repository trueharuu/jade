use fumen::Fumen;

fn main() {
    let f = Fumen::decode("v115@9gilFewhglAtGewhBtR4Rpi0whAtR4AeRpBeg0whJe?AgH").unwrap();
    println!("pages: {}", f.pages.len());
    for (i, p) in f.pages.iter().enumerate() {
        println!("page {}: piece={:?}", i, p.piece);
    }
}
