// 1802 disassembler

use std::{fs::File, io::Read, path::PathBuf};

use clap::Parser;
use color_eyre::{eyre, Result};
use itertools::Itertools;

use crate::{
    chips::cdp1802::MemoryRange,
    cli::{parse_addr, parse_memory_range},
    instr::{Instr, InstrSchema},
};

#[derive(Parser, Debug)]
pub struct DisArgs {
    /// Binary file to disassemble.
    #[arg()]
    file: PathBuf,

    /// Address at which the binary is loaded.
    #[arg(long, value_parser=parse_addr, default_value="0x0000")]
    base: u16,

    /// Memory range.
    #[arg(long, value_parser=parse_memory_range)]
    range: Option<MemoryRange>,
}

pub fn run(args: DisArgs) -> Result<()> {
    let mut file = File::open(&args.file)?;
    let mut data = vec![];
    file.read_to_end(&mut data)?;
    if data.is_empty() {
        eyre::bail!("no data");
    }

    let base = usize::from(args.base);
    let mut start = base;
    let mut end = (base + data.len().saturating_sub(1)).min(0xffff);

    if let Some(range) = args.range {
        let mut clamped = false;
        if let Some(addr) = range.start {
            let addr = usize::from(addr);
            if addr < start {
                clamped = true;
            } else {
                start = addr;
            }
        }
        if let Some(addr) = range.end {
            let addr = usize::from(addr);
            if addr > end {
                clamped = true;
            } else {
                end = addr;
            }
        }
        if clamped {
            eprintln!("clamping --range to {start:#06x}..{end:#06x}");
        }
    }

    assert!(base <= start);
    assert!(start <= end);

    let mut offset = start - base;
    let limit = end - base;
    while offset < limit {
        let instr_limit = (offset + 2).min(limit);
        let instr = Instr::decode(&data[offset..=instr_limit]);
        let (bytes, disasm, size) = match instr {
            Some(instr) => (
                instr
                    .encode()
                    .into_iter()
                    .map(|v| format!("{v:02x}"))
                    .join(" "),
                instr.disasm(),
                instr.size(),
            ),
            None => (format!("{:02x}", data[offset]), "??".to_string(), 1),
        };
        println!("{:04x} {bytes:<8} {disasm}", offset + base);
        offset += size as usize;
    }

    Ok(())
}
