% covers: 10 - a value a roundoff away from an integer displays with decimals, as det([1 2; 3 4]) does in MATLAB
% The display half of item 10's det line. Its value half, that det returns a
% value a roundoff away from -2 at all, is cycle 08's (verify first): this
% det lands exactly on -2, so det itself is not called here. -2 - eps(2) is the
% neighbouring double below -2.
x = -2 - eps(2)
