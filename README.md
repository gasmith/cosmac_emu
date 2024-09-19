# COSMAC Emulator

This is an emulator for the RCA 1802 "COSMAC" microprocessor.

## Project status

I work on this from time to time, usually when I have the itch to futz with the
[1802 membership card](http://www.retrotechnology.com/memship/memship.html).
It's useful for testing out ideas before going the hassle of loading them into
the device, but it's still pretty clunky.

## Usage

To load an image:

```console
$ cargo run -- --image stack-test.bin
00000000 d=00.1 x=00:0000:f8 p=00:0000:f8 08    ldi  8
>>
```

For testing that involves handling external events, you can inject an event
file, containing rows of `time_ns,event_type[,payload]`, and specify the
duration of each machine cycle:

```console
$ cat events.evlog
110200,flag,ef3,0
232920,flag,ef3,1
593003,int
969022,input,io5,fb

$ cargo run -- --image event-test.bin --event-log events.evlog --cycle-time 2us
00000000 d=00.0 x=00:0000:f8 p=00:0000:f8 08    ldi  8
>>
```

## To Do

- The CLI leaves much to be desired.
  - `clap` and `clap-repl` are _just OK_ for interactive CLIs.
  - The commands themselves are not terribly consistent or well-documented.
- Add better hooks for testing.
  - For example, to validate that a PLL transmitter generates a correctly-timed
    waveform on Q.
  - Maybe write a log file of externally visible events?
- Listing/symbolic integration would be super nice.

