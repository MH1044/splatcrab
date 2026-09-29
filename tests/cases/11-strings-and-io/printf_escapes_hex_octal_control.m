% covers: 6 - (Scope, QA D16 e) the escapes \xN, \N (octal), \a, \b, \f and \v are processed, by sprintf and by fprintf
% \x41 is A, \102 is B and \x43 is C; \a, \b, \f and \v are the code points
% 7, 8, 12 and 11.
disp(sprintf('\x41\102'))
disp(double(sprintf('\a\b\f\v')))
fprintf('\x43\n')
