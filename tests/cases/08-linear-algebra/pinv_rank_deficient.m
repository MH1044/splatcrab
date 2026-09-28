% covers: 7 - pinv of a singular matrix is its Moore-Penrose pseudoinverse
% [1 2; 2 4] is 5*u*u' with u = [1; 2]/sqrt(5), so pinv is u*u'/5, which is
% [1 2; 2 4]/25: 0.04, 0.08, 0.08 and 0.16 in column-major order.
fprintf('%.4f ', pinv([1 2; 2 4])); fprintf('\n');
