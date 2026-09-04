"""The Computer Language Benchmarks Game reverse-complement benchmark.

Contributed by Jacob Lee, Steven Bethard, et al.; adapted for the Rapira26
benchmark suite.  Input is read from stdin so Python and Rapira process the
same FASTA fixture.
"""

import sys


def show(header, sequence, table=bytes.maketrans(
        b"ACBDGHKMNSRUTWVYacbdghkmnsrutwvy",
        b"TGVHCDMKNSYAAWBRTGVHCDMKNSYAAWBR"),
        write=sys.stdout.buffer.write, nl=b"\n"):
    sequence = sequence.translate(table)
    sequence.reverse()

    write(header + nl)
    for index in range(0, len(sequence), 60):
        write(sequence[index:index + 60] + nl)


def main():
    header = None
    sequence = bytearray()

    # Grow one sequence as the buffered input iterator supplies each line.  In
    # particular, do not inspect the input size and preallocate the full file.
    for line in sys.stdin.buffer:
        line = line.removesuffix(b"\n")
        if line.startswith(b">"):
            if header is not None:
                show(header, sequence)
            header = line
            sequence = bytearray()
        else:
            sequence.extend(line)

    if header is not None:
        show(header, sequence)


main()
