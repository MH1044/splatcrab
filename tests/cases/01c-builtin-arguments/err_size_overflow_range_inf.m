% covers: 11 - a range whose count overflows f64 reports 1xInf; exit 1, not 101
x = 0:1e-300:1e300;
