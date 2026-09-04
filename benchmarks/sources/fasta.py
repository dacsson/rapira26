"""The Computer Language Benchmarks Game FASTA benchmark.

Naive transliteration from Drake Diedrich's C program, contributed by
Isaac Gouy. This character-at-a-time version is also used to generate input
for the reverse-complement benchmark.
"""

from sys import argv

IM = 139968
IA = 3877
IC = 29573
SEED = 42

seed = SEED


def fastaRand(maximum):
    global seed
    seed = (seed * IA + IC) % IM
    return maximum * seed / IM


ALU = (
    "GGCCGGGCGCGGTGGCTCACGCCTGTAATCCCAGCACTTTGG"
    "GAGGCCGAGGCGGGCGGATCACCTGAGGTCAGGAGTTCGAGA"
    "CCAGCCTGGCCAACATGGTGAAACCCCGTCTCTACTAAAAAT"
    "ACAAAAATTAGCCGGGCGTGGTGGCGCGCGCCTGTAATCCCA"
    "GCTACTCGGGAGGCTGAGGCAGGAGAATCGCTTGAACCCGGG"
    "AGGCGGAGGTTGCAGTGAGCCGAGATCGCGCCACTGCACTCC"
    "AGCCTGGGCGACAGAGCGAGACTCCGTCTCAAAAA"
)

IUB = "acgtBDHKMNRSVWY"
IUB_P = [
    0.27, 0.12, 0.12, 0.27,
    0.02, 0.02, 0.02, 0.02, 0.02, 0.02, 0.02, 0.02, 0.02, 0.02, 0.02,
]

HomoSapiens = "acgt"
HomoSapiens_P = [
    0.3029549426680,
    0.1979883004921,
    0.1975473066391,
    0.3015094502008,
]

LINELEN = 60


def repeatFasta(seq, n):
    length = len(seq)
    i = 0
    # Explicit line buffer: intentionally character-at-a-time.
    buffer = ""
    for i in range(0, n):
        buffer += seq[i % length]
        if i % LINELEN == LINELEN - 1:
            print(buffer)
            buffer = ""
    if i % LINELEN != 0:
        print(buffer)


def randomFasta(seq, probability, n):
    length = len(seq)
    i, j = 0, 0
    # Explicit line buffer: intentionally character-at-a-time.
    buffer = ""
    for i in range(0, n):
        value = fastaRand(1.0)
        # Slowest idiomatic linear lookup. Fast when the alphabet is short.
        for j in range(0, length):
            value -= probability[j]
            if value < 0:
                break
        buffer += seq[j]
        if i % LINELEN == LINELEN - 1:
            print(buffer)
            buffer = ""
    if (i + 1) % LINELEN != 0:
        print(buffer)


def main(n):
    print(">ONE Homo sapiens alu")
    repeatFasta(ALU, n * 2)

    print(">TWO IUB ambiguity codes")
    randomFasta(IUB, IUB_P, n * 3)

    print(">THREE Homo sapiens frequency")
    randomFasta(HomoSapiens, HomoSapiens_P, n * 5)


if __name__ == "__main__":
    main(int(argv[1]) if len(argv) > 1 else 1000)
