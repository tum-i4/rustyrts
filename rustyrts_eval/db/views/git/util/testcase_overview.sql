-- this view just joins TestReport, TestSuite (retest-all, dynamic, static) and TestCase
-- only if these are comparable
--CREATE MATERIALIZED VIEW testcase_overview AS
SELECT report.commit,
       report.retest_all_id                 as retest_all_id,
       report.dynamic_id                    as dynamic_id,
       report.static_id                     as static_id,

       retest_all_test_cases.target         as target,

       retest_all_test_cases.testsuite_name as retest_all_suite_name,
       retest_all_test_cases.name           as retest_all_name,
       retest_all_test_cases.id             as retest_all_testcase_id,
       retest_all_test_cases.status         as retest_all_testcase_status,

       dynamic_test_cases.testsuite_name    as dynamic_suite_name,
       dynamic_test_cases.name              as dynamic_name,
       dynamic_test_cases.id                as dynamic_testcase_id,
       dynamic_test_cases.status            as dynamic_testcase_status,

       static_test_cases.testsuite_name     as static_suite_name,
       static_test_cases.name               as static_name,
       static_test_cases.id                 as static_testcase_id,
       static_test_cases.status             as static_testcase_status

FROM ((testreport_extended report
    join testcase_extended retest_all_test_cases
       on report.retest_all_id = retest_all_test_cases.report_id)
    left outer join testcase_extended dynamic_test_cases
      on report.dynamic_id = dynamic_test_cases.report_id
          and retest_all_test_cases.name = dynamic_test_cases.name
          and retest_all_test_cases.testsuite_name = dynamic_test_cases.testsuite_name)
         left outer join testcase_extended static_test_cases
                         on report.static_id = static_test_cases.report_id
                             and retest_all_test_cases.name = static_test_cases.name
                             and retest_all_test_cases.testsuite_name = static_test_cases.testsuite_name

WHERE retest_all_test_cases.crashed = false -- filter suites that are not comparable
  and (dynamic_test_cases.crashed is null or dynamic_test_cases.crashed = false)
  and (static_test_cases.crashed is null or static_test_cases.crashed = false)
  and retest_all_test_cases.status != 'IGNORED'
--;