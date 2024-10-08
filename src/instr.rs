use cosmac_emu_macros::InstrSchema;
use itertools::Itertools;

/// The interface for a instruction.
pub trait InstrSchema: Sized {
    /// Decodes an instruction from bytes.
    fn decode(bin: &[u8]) -> Option<Self>;
    /// Returns a string representation of the instruction.
    fn disasm(&self) -> String;
    /// Encodes the instruction into bytes.
    fn encode(&self) -> Vec<u8>;
    /// Returns the size of the instruction, in bytes.
    fn size(&self) -> u8;
    /// Returns a "listing", with the encoded and disassembled instruction side-by-side.
    fn listing(&self) -> String {
        let enc = self
            .encode()
            .into_iter()
            .map(|v| format!("{v:02x}"))
            .join(" ");
        let disasm = self.disasm();
        format!("{enc:<8} {disasm}")
    }
    /// Returns the opcode for this instruction, including packed data.
    fn opcode(&self) -> u8 {
        self.encode()[0]
    }
}

#[derive(Clone, Copy, InstrSchema)]
pub enum Instr {
    #[schema("00")]
    Idl,
    #[schema("0n")]
    Ldn(u8),
    #[schema("1n")]
    Inc(u8),
    #[schema("2n")]
    Dec(u8),
    #[schema("30 nn")]
    Br(u8),
    #[schema("31 nn")]
    Bq(u8),
    #[schema("32 nn")]
    Bz(u8),
    #[schema("33 nn")]
    Bdf(u8),
    #[schema("34 nn")]
    B1(u8),
    #[schema("35 nn")]
    B2(u8),
    #[schema("36 nn")]
    B3(u8),
    #[schema("37 nn")]
    B4(u8),
    #[schema("38")]
    Skp,
    #[schema("39 nn")]
    Bnq(u8),
    #[schema("3a nn")]
    Bnz(u8),
    #[schema("3b nn")]
    Bnf(u8),
    #[schema("3c nn")]
    Bn1(u8),
    #[schema("3d nn")]
    Bn2(u8),
    #[schema("3e nn")]
    Bn3(u8),
    #[schema("3f nn")]
    Bn4(u8),
    #[schema("4n")]
    Lda(u8),
    #[schema("5n")]
    Str(u8),
    #[schema("60")]
    Irx,
    #[schema("6l")]
    Out(u8),
    #[schema("68")]
    Resv68,
    #[schema("6h")]
    Inp(u8),
    #[schema("70")]
    Ret,
    #[schema("71")]
    Dis,
    #[schema("72")]
    Ldxa,
    #[schema("73")]
    Stxd,
    #[schema("74")]
    Adc,
    #[schema("75")]
    Sdb,
    #[schema("76")]
    Shrc,
    #[schema("77")]
    Smb,
    #[schema("78")]
    Sav,
    #[schema("79")]
    Mark,
    #[schema("7a")]
    Req,
    #[schema("7b")]
    Seq,
    #[schema("7c nn")]
    Adci(u8),
    #[schema("7d nn")]
    Sdbi(u8),
    #[schema("7e")]
    Shlc,
    #[schema("7f nn")]
    Smbi(u8),
    #[schema("8n")]
    Glo(u8),
    #[schema("9n")]
    Ghi(u8),
    #[schema("an")]
    Plo(u8),
    #[schema("bn")]
    Phi(u8),
    #[schema("c0 hh ll")]
    Lbr(u8, u8),
    #[schema("c1 hh ll")]
    Lbq(u8, u8),
    #[schema("c2 hh ll")]
    Lbz(u8, u8),
    #[schema("c3 hh ll")]
    Lbdf(u8, u8),
    #[schema("c4")]
    Nop,
    #[schema("c5")]
    Lsnq,
    #[schema("c6")]
    Lsnz,
    #[schema("c7")]
    Lsnf,
    #[schema("c8")]
    Lskp,
    #[schema("c9 hh ll")]
    Lbnq(u8, u8),
    #[schema("ca hh ll")]
    Lbnz(u8, u8),
    #[schema("cb hh ll")]
    Lbnf(u8, u8),
    #[schema("cc")]
    Lsie,
    #[schema("cd")]
    Lsq,
    #[schema("ce")]
    Lsz,
    #[schema("cf")]
    Lsdf,
    #[schema("dn")]
    Sep(u8),
    #[schema("en")]
    Sex(u8),
    #[schema("f0")]
    Ldx,
    #[schema("f1")]
    Or,
    #[schema("f2")]
    And,
    #[schema("f3")]
    Xor,
    #[schema("f4")]
    Add,
    #[schema("f5")]
    Sd,
    #[schema("f6")]
    Shr,
    #[schema("f7")]
    Sm,
    #[schema("f8 nn")]
    Ldi(u8),
    #[schema("f9 nn")]
    Ori(u8),
    #[schema("fa nn")]
    Ani(u8),
    #[schema("fb nn")]
    Xri(u8),
    #[schema("fc nn")]
    Adi(u8),
    #[schema("fd nn")]
    Sdi(u8),
    #[schema("fe")]
    Shl,
    #[schema("ff nn")]
    Smi(u8),
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_out_inp() {
        assert!(matches!(Instr::decode(&[0x60]), Some(Instr::Irx)));
        assert!(matches!(Instr::decode(&[0x61]), Some(Instr::Out(1))));
        assert!(matches!(Instr::decode(&[0x67]), Some(Instr::Out(7))));
        assert!(matches!(Instr::decode(&[0x68]), Some(Instr::Resv68)));
        assert!(matches!(Instr::decode(&[0x69]), Some(Instr::Inp(1))));
        assert!(matches!(Instr::decode(&[0x6f]), Some(Instr::Inp(7))));
    }
}
