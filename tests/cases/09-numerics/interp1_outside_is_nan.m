% covers: 3 - interp1 at a point outside the sample range is NaN: no extrapolation by default
disp(interp1([1 2 3], [10 20 30], 0.5))
