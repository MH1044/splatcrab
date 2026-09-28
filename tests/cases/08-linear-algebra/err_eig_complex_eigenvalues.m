% covers: 12 - eig of a real matrix whose eigenvalues are complex is the clean refusal until cycle 10, exit 1, never a wrong real answer
% A rotation by 90 degrees has eigenvalues i and -i. The message begins with
% the prefix cycle 01d's refusals share; the Error: prefix names line 5,
% counting this covers line as line 1.
eig([0 -1; 1 0])
