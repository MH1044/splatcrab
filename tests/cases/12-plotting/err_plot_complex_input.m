% covers: 1 - (Scope, complex values) plot of a complex vector is refused by cycle 10's gate with its message, exit 1
% NOTE: MATLAB plots the real part against the imaginary part; SplatCrab
% NOTE: refuses a complex argument, the deviation the spec records.
% The text is cycle 10's, as tests/cases/10-complex/err_floor_complex_input.err
% has it. The gate refuses before plot runs, so nothing is drawn or written.
plot([1+2i 3]);
