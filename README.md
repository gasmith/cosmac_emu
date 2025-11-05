# COSMAC Emulator

This is an emulator for the RCA 1802 "COSMAC" microprocessor.

## Project status

I work on this from time to time, usually when I have the itch to futz with the
[1802 membership card](http://www.retrotechnology.com/memship/memship.html).
It's useful for testing out ideas before going the hassle of loading them into
the device, but it's still pretty clunky.

## Usage

### TUI

To run a ROM in the TUI:

```console
$ cargo run -- tui --rom MS20ANSJ.bin --jump-to 0x8000
┌Terminal────────────────────────────────────────────────────────────────────────┐┌Front Panel───────────────────┐
│                                                                                ││  output ○ ○ ○ ○ ○ ○ ○ ○ 00   │
│Membership Card MS20ANSJ Monitor v2.0JR 11 July 2023 by Chuck Yakym.            ││   input ○ ○ ○ ○ ○ ○ ○ ○ 00   │
│Enter "H" for Help.                                                             ││ in ○  wait ○  clr ○  read ○  │
│>H                                                                              │└──────────────────────────────┘
│Commands                Description                                             │┌Registers─────────────────────┐
│--------                -----------                                             ││ d=3e.1 p=3 x=2 t=0000 in=3e  │
│H                       Help                                                    ││ 0=8bf3 1=0000 2=7fbb 3=8006  │
│B                       BASIC level 3 v1.1                                      ││ 4=8adb 5=8aed 6=80a6 7=8cc4  │
│P                       Play Adventureland                                      ││ 8=0000 9=0000 a=0000 b=ff3e  │
│L                       Load program or data (Intel HEX format                  ││ c=0000 d=0000 e=011a f=3e00  │
│V                       View 1802 registers                                     │└──────────────────────────────┘
│Daaaa bbbb<CR>          Disassemble Opcodes from aaaa to bbbb                   │┌Listing───────────────────────┐
│Maaaa bbbb<CR>          Memory read from aaaa for bbbb bytes                    ││ >8005 3e 05    bn3  5        │
│Waaaa dd dd..<CR>       Write to memory until <CR>                              ││  8007 8e       glo  14       │
│Saaaa bbbb<CR>          Save memory at aaaa for bbbb bytes (Intel HEX format)   ││  8008 f6       shr           │
│Taaaa bbbb cccc<CR>     Transfer (copy) memory from aaaa to bbbb for cccc bytes ││  8009 ff 02    smi  2        │
│Raaaa<CR>               Run program with R0=aaaa P=0 X=0 Q=0                    ││  800b ff 01    smi  1        │
│                                                                                ││  800d 3a 0b    bnz  11       │
│All commands are UPPERCASE. All numbers are HEX. <ESC> aborts command.          ││  800f 8e       glo  14       │
│                                                                                ││  8010 ff 01    smi  1        │
│>                                                                               ││  8012 ff 01    smi  1        │
│                                                                                ││  8014 3a 12    bnz  18       │
│                                                                                ││  8016 8b       glo  11       │
│                                                                                ││  8017 3e 1b    bn3  27       │
│                                                                                ││  8019 36 1d    b3   29       │
└────────────────────────────────────────────────────────────────────────────────┘└──────────────────────────────┘
```

You may need to fiddle with the UART baud, negate Q or EF3, and set the ROM
address for certain images:

```console
$ cargo run -- tui --rom disklessElfOS.bin@0x0000 --invert-q --uart-baud 7200
```

### Debugger

To load an image:

```console
$ cargo run -- dbg --ram stack-test.bin
00000000 d=00.1 x=00:0000:f8 p=00:0000:f8 08    ldi  8
>>
```

For testing that involves handling external events, you can inject an event
file, containing rows of `time_ns,event_type[,payload]`, and specify the
clock frequency:

```console
$ cat events.evlog
110200,flag,ef3,0
232920,flag,ef3,1
593003,int
969022,input,io5,fb

$ cargo run -- dbg --ram event-test.bin --event-log events.evlog --clock-freq 1.8mhz
00000000 d=00.0 x=00:0000:f8 p=00:0000:f8 08    ldi  8
>>
```
