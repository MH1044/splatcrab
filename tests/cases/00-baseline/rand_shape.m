% covers: rand produces the requested shape with values in [0, 1); no value is asserted
r = rand(2, 3);
disp(size(r))
disp(all(r(:) >= 0 & r(:) < 1))
disp(numel(rand(4)))
