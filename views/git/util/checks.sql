-- this file contains some sanity checks

CREATE VIEW check_parsed_tests_mutants as
-- we check that the number of parsed test cases equals the number of actually executed tests
SELECT s.*, count(c.id) as count_cases, s.passed_count + s.failed_count + s.measured_count - count(c.id) as deviation
FROM "TestSuite" s,
     "TestCase" c
WHERE s.id = c.suite_id
  AND c.status != 'IGNORED'
GROUP BY s.id, s.passed_count, s.failed_count, s.measured_count
HAVING count(c.id) != s.passed_count + s.failed_count + s.measured_count;