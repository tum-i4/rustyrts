CREATE MATERIALIZED VIEW mutant_testcase_overview
AS
SELECT mutant.commit,
       mutant.descr                 as descr,
       mutant.retest_all_id         as retest_all_mutant_id,
       mutant.dynamic_id            as dynamic_mutant_id,
       mutant.static_id             as static_mutant_id,

       retest_all_test_cases.name   as retest_all_name,
       retest_all_test_cases.id     as retest_all_testcase_id,
       retest_all_test_cases.status as retest_all_testcase_status,

       dynamic_test_cases.name      as dynamic_name,
       dynamic_test_cases.id        as dynamic_testcase_id,
       dynamic_test_cases.status    as dynamic_testcase_status,

       static_test_cases.name       as static_name,
       static_test_cases.id         as static_testcase_id,
       static_test_cases.status     as static_testcase_status

FROM ((mutant_extended mutant
    join mutant_testcase_extended retest_all_test_cases
       on mutant.retest_all_id = retest_all_test_cases.mutant_id)
    left outer join mutant_testcase_extended dynamic_test_cases
      on mutant.dynamic_id = dynamic_test_cases.mutant_id
          and retest_all_test_cases.name = dynamic_test_cases.name
          and retest_all_test_cases.testsuite_name = dynamic_test_cases.testsuite_name)
         left outer join mutant_testcase_extended static_test_cases
                         on mutant.static_id = static_test_cases.mutant_id
                             and retest_all_test_cases.name = static_test_cases.name
                             and retest_all_test_cases.testsuite_name = static_test_cases.testsuite_name;