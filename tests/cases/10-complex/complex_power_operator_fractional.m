% covers: 10 - a negative number to a fractional power through ^ is the principal root: (-8)^(1/3) is 1 + 1.7321i
% Replaces 01d's err_complex_power_operator, the refusal of the same input.
z = (-8)^(1/3); fprintf('%.4f %.4f\n', real(z), imag(z))
