% covers: 3 - a parse error names the token as a human writes it: ';', not Semi
% The 01b spec's own example renders this `unexpected ';' in expression`;
% `Token`'s Debug name reaches the user today instead.
y = x + ;
