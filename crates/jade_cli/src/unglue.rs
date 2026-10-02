// port of marfung37's `unglue.py`.

pub fn unglue(fumen: &str) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::unglue;
    
    fn test(fumen: &str, expected: &str) {
        let result = unglue(fumen);
        assert_eq!(result, expected);
    }

    #[test]
    fn empty() {
        test("v115@vhAAgh", "v115@vhAAgh");
    }

    #[test]
    fn ijlo_box() {
        test("v115@vhDSSJznBGjBJnB", "v115@Dhwhi0FewhRpg0FewhRpglFewhilJeAgH");
    }

    #[test]
    fn jigsaw() {
        test("v115@vhFMJJXqBifBpoBTrBmsB", "v115@9gilFewhglAtGewhBtR4Rpi0whAtR4AeRpBeg0whJe?AgH");
    }

    #[test]
    fn pco_solve() {
        test("v115@9gD8DeF8CeG8BeH8CeC8Je0LJvhBXnBOrB", "v115@9gD8h0R4F8g0R4G8BtH8g0BtC8JeAgH");
    }
}