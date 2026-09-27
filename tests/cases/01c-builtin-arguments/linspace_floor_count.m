% covers: 18 - linspace floors a non-integer count (QA D23), and a count below 1 gives 1x0
% NOTE: the interior points of linspace(2, 8, 4.99) go through fprintf so that
% NOTE: an ulp of roundoff in them cannot change the display format.
disp(size(linspace(0, 1, 2.7)))
disp(linspace(0, 1, 2.7))
disp(size(linspace(0, 1, 0.5)))
disp(linspace(0, 10, 3.9))
disp(linspace(0, 1, 1.5))
disp(size(linspace(0, 1, -2.5)))
disp(size(linspace(2, 8, 4.99)))
fprintf('%.4f %.4f %.4f %.4f\n', linspace(2, 8, 4.99));
