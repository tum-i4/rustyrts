CREATE VIEW testcase_extended
AS
SELECT testcase.*,
       testsuite.crashed,
       testsuite.name as testsuite_name,
       testsuite.report_id
FROM "TestCase" testcase,
     "TestSuite" testsuite
WHERE testcase.suite_id = testsuite.id;