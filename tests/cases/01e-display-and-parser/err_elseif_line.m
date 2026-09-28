% covers: 5 - an error in an elseif condition names the elseif's own line
% QA D27: the line is attached by `exec_block` from the `if` statement that is
% still unwinding, so the report names the `if`'s line instead. The marker and
% these comment lines are physical lines 1 to 7, so `if 0` is line 8 and
% `elseif undefined_d` is line 9. Only the `Line N:` prefix is asserted, as
% 01b-error-reporting/err_line_parse.err does, so this case stays independent
% of the undefined-name wording that bullet 15 pins.
if 0
elseif undefined_d
end
