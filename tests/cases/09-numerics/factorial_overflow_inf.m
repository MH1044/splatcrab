% covers: 10 - (Scope, factorial) 170! is the largest factorial a double holds; 171! is Inf, and so is factorial(1e15), with no 1e15-step loop
% 170! is about 7.3e306 and 171! about 1.2e309, past realmax. The third
% call must return within the harness's timeout, so it cannot multiply
% its way there.
disp(factorial(170) < Inf); disp(factorial(171)); disp(factorial(1e15))
