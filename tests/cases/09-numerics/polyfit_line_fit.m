% covers: 1 - polyfit of degree 1 through three collinear points gives the line's slope 2 and intercept 0
% abs keeps a roundoff -0.0000 intercept from printing its sign.
p = polyfit([1 2 3], [2 4 6], 1); fprintf('%.4f %.4f\n', abs(p));
