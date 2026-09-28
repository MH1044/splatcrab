% covers: 3 - the same for a number, and for a token a message expects: ')', not RParen
% Whitespace separates elements inside [ ] only, so the second number in this
% argument list is simply unexpected. Today the message is
% `expected RParen but found Num(0.3)`; both halves of it are Debug names, and
% both are asserted here in the form a human writes:
%   expected ')' but found '0.3'
% A token's text is quoted whatever the token is, which is what keeps one rule
% for the whole display form.
disp(0.3 0.3)
