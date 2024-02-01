-- this view calculates how often a test has been selected or failed
--create materialized view target_count AS
SELECT overview.commit,
       overview.target,
       overview.retest_all_mutant_id,
       overview.descr                                     as descr,

       -- we use distinct here in case there are multiple tests with same suite name and testcase name

       count(distinct overview.retest_all_testcase_id)    AS retest_all_count,

       count(distinct (SELECT testcase.id
                       FROM "MutantsTestCase" testcase
                       WHERE testcase.id = overview.retest_all_testcase_id
                         AND testcase.status = 'FAILED')) AS retest_all_count_failed,

       count(distinct overview.dynamic_testcase_id)       AS dynamic_count,

       count(distinct (SELECT testcase.id
                       FROM "MutantsTestCase" testcase
                       WHERE testcase.id = overview.dynamic_testcase_id
                         AND testcase.status = 'FAILED')) AS dynamic_count_failed,

       count(distinct overview.static_testcase_id)        AS static_count,

       count(distinct (SELECT testcase.id
                       FROM "MutantsTestCase" testcase
                       WHERE testcase.id = overview.static_testcase_id
                         AND testcase.status = 'FAILED')) AS static_count_failed


FROM mutant_testcase_overview overview
GROUP BY overview.commit, overview.target, overview.descr, overview.retest_all_mutant_id, overview.dynamic_mutant_id,
         overview.static_mutant_id
--;