% covers: 15 - the error-text policy: current MATLAB's wording for an unknown name
% QA D33. The policy recorded for this cycle is to match the message texts of
% MATLAB R2020a and later -- the release the Known bugs table names -- rather
% than to keep the older wording deliberately. A MATLAB-compatible interpreter
% that reports a sentence MATLAB stopped printing six releases ago is the
% harder position to defend, and the row is worded as a defect rather than as
% a choice.
%
% So `Undefined function or variable 'x'.` becomes
%   Unrecognized function or variable 'zzz_not_defined'.
% throughout error.rs, and every .err file that quotes the old wording changes
% with it (00-baseline/err_undefined.err among them), justified in the commit
% body as the spec requires for a case outside this module's directory.
disp(zzz_not_defined)
