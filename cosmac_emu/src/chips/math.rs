pub fn add(x: u8, y: u8) -> (u8, bool) {
    x.overflowing_add(y)
}

pub fn addc(x: u8, y: u8, df: bool) -> (u8, bool) {
    let (acc, df1) = add(x, y);
    let (acc, df2) = add(acc, df.into());
    (acc, df1 || df2)
}

pub fn sub(x: u8, y: u8) -> (u8, bool) {
    let (z, overflow) = x.overflowing_sub(y);
    (z, !overflow)
}

pub fn subc(x: u8, y: u8, df: bool) -> (u8, bool) {
    let (acc, df1) = sub(x, y);
    let (acc, df2) = sub(acc, df.into());
    (acc, df1 && df2)
}

#[cfg(test)]
mod test {
    use super::{add, addc, sub, subc};

    #[test]
    fn test_add() {
        assert_eq!(add(0, 0), (0, false));
        assert_eq!(add(0, 1), (1, false));
        assert_eq!(add(0xfe, 1), (0xff, false));
        assert_eq!(add(0xff, 1), (0, true));
        assert_eq!(add(0xff, 0xff), (0xfe, true));
    }

    #[test]
    fn test_addc() {
        assert_eq!(addc(0, 0, false), (0, false));
        assert_eq!(addc(0, 0, true), (1, false));
        assert_eq!(addc(0, 1, true), (2, false));
        assert_eq!(addc(1, 1, true), (3, false));
        assert_eq!(addc(0xff, 0, true), (0, true));
        assert_eq!(addc(0xff, 1, false), (0, true));
        assert_eq!(addc(0xfe, 1, true), (0, true));
        assert_eq!(addc(0xff, 1, true), (1, true));
        assert_eq!(addc(0xff, 2, true), (2, true));
    }

    #[test]
    fn test_sub() {
        assert_eq!(sub(0, 0), (0, true));
        assert_eq!(sub(0, 1), (0xff, false));
        assert_eq!(sub(1, 0), (1, true));
        assert_eq!(sub(0xff, 1), (0xfe, true));
        assert_eq!(sub(1, 0xff), (2, false));
    }

    #[test]
    fn test_subc() {
        assert_eq!(subc(0, 0, false), (0, true));
        assert_eq!(subc(1, 1, false), (0, true));
        assert_eq!(subc(1, 0, true), (0, true));
        assert_eq!(subc(1, 1, true), (0xff, false));
        assert_eq!(subc(0, 0, true), (0xff, false));
        assert_eq!(subc(0, 1, false), (0xff, false));
        assert_eq!(subc(0, 1, true), (0xfe, false));
    }
}
