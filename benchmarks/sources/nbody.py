"""The equivalent N-body benchmark, using stdin to match the Rapira port.

Originally by Kevin Carson; modified by Tupteq, Fredrik Johansson,
Daniel Nanz, and Maciej Fijalkowski; adapted for the Rapira26 benchmark suite.
"""

import sys


PI = 3.14159265358979323
SOLAR_MASS = 4.0 * PI * PI
DAYS_PER_YEAR = 365.24

SYSTEM = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, SOLAR_MASS],
    [4.84143144246472090, -1.16032004402742839, -0.103622044471123109,
     0.00166007664274403694 * DAYS_PER_YEAR, 0.00769901118419740425 * DAYS_PER_YEAR,
     -0.0000690460016972063029 * DAYS_PER_YEAR, 0.000954791938424326609 * SOLAR_MASS],
    [8.34336671824457987, 4.12479856412430479, -0.403523417114321381,
     -0.00276742510726862411 * DAYS_PER_YEAR, 0.00499852801234917238 * DAYS_PER_YEAR,
     0.0000230417291729535919 * DAYS_PER_YEAR, 0.000285885980666130812 * SOLAR_MASS],
    [12.8943695621391310, -15.1111514016986312, -0.223307578892655734,
     0.00296460137564761618 * DAYS_PER_YEAR, 0.00237847173959480950 * DAYS_PER_YEAR,
     -0.0000296589568540237556 * DAYS_PER_YEAR, 0.0000436624404335156298 * SOLAR_MASS],
    [15.3796911485091467, -25.9193146099879641, 0.179258772950371181,
     0.00268067772490389322 * DAYS_PER_YEAR, 0.00162824170038242295 * DAYS_PER_YEAR,
     -0.0000951592254519715870 * DAYS_PER_YEAR, 0.0000515138902046611451 * SOLAR_MASS],
]
PAIRS = ((0, 1), (0, 2), (0, 3), (0, 4), (1, 2),
         (1, 3), (1, 4), (2, 3), (2, 4), (3, 4))


def advance(system, pairs, dt, count):
    for _ in range(count):
        for index1, index2 in pairs:
            body1 = system[index1]
            body2 = system[index2]
            dx = body1[0] - body2[0]
            dy = body1[1] - body2[1]
            dz = body1[2] - body2[2]
            mag = dt * ((dx * dx + dy * dy + dz * dz) ** -1.5)
            mass1 = body1[6] * mag
            mass2 = body2[6] * mag
            body1[3] -= dx * mass2
            body1[4] -= dy * mass2
            body1[5] -= dz * mass2
            body2[3] += dx * mass1
            body2[4] += dy * mass1
            body2[5] += dz * mass1
        for body in system:
            body[0] += dt * body[3]
            body[1] += dt * body[4]
            body[2] += dt * body[5]


def energy(system, pairs):
    result = 0.0
    for index1, index2 in pairs:
        body1 = system[index1]
        body2 = system[index2]
        dx = body1[0] - body2[0]
        dy = body1[1] - body2[1]
        dz = body1[2] - body2[2]
        result -= (body1[6] * body2[6]) / ((dx * dx + dy * dy + dz * dz) ** 0.5)
    for body in system:
        result += body[6] * (body[3] * body[3] + body[4] * body[4] + body[5] * body[5]) / 2.0
    return result


def offset_momentum(system):
    px = py = pz = 0.0
    for body in system:
        px -= body[3] * body[6]
        py -= body[4] * body[6]
        pz -= body[5] * body[6]
    sun = system[0]
    sun[3] = px / sun[6]
    sun[4] = py / sun[6]
    sun[5] = pz / sun[6]


def main(count):
    offset_momentum(SYSTEM)
    print(format(energy(SYSTEM, PAIRS), ".16g"))
    advance(SYSTEM, PAIRS, 0.01, count)
    print(format(energy(SYSTEM, PAIRS), ".16g"))


main(int(sys.stdin.readline()))
