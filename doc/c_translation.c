#include <math.h>
#include <stdbool.h>
#include <stdio.h>

bool PROSTOE(int N) {
  if (N < 2) {
    return false;
  }

  for(double M = 2; M < (sqrt(N) + 0.5); M++) {
    if (N % (int)M == 0) {
      return false;
    }
  }

  return true;
}

int main() {
  printf("%d\n", PROSTOE(2003));
  printf("%d\n", PROSTOE(2004));
}
