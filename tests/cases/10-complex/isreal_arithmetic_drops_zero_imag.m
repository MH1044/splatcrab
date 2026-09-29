% covers: 7 - an arithmetic result whose imaginary parts are all zero is stored real: 1i * 0, and the sum of 3+4i and 5-4i
% From MathWorks' complex and isreal pages, as the spec's Scope records them.
disp(isreal(1i * 0)); disp(isreal((3+4i) + (5-4i)))
