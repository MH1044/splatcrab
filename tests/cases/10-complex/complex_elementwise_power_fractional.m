% covers: 10 - the same principal root through the element-wise .^ operator
% Replaces 01d's err_complex_elementwise_power, the refusal of the same input.
z = (-8).^(1/3); fprintf('%.4f %.4f\n', real(z), imag(z))
