#!/usr/bin/env python3

n = 5_000_000
mod = 1_000_000_007
a, b = 0, 1
for _ in range(n):
    a, b = b, (a + b) % mod
print(a)
