% covers: 10 - a scalar outside the fixed-point range displays in e-notation (QA D20)
% Values are the spec's recorded ones; the spec notes they await a run against
% real MATLAB. 1e10 is an integer but still too wide for the integer format.
x = 1234.5
x = 12345.6
x = 0.001
x = 1e10
