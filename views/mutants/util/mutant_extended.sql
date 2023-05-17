CREATE VIEW mutant_extended
AS
SELECT c.id                          as commit,
       c.commit_str,
       c.repo_id,

       retest_all_mutant.descr       as descr,

       retest_all_mutant.id          as retest_all_id,
       retest_all_mutant.test_log    as retest_all_test_log,
       retest_all_mutant.test_result as retest_all_test_result,

       dynamic_mutant.id             as dynamic_id,
       dynamic_mutant.test_log       as dynamic_test_log,
       dynamic_mutant.test_result    as dynamic_test_result,

       static_mutant.id              as static_id,
       static_mutant.test_log        as static_test_log,
       static_mutant.test_result     as static_test_result

FROM "Commit" c,
     "MutantsReport" retest_all,
     "MutantsReport" dynamic,
     "MutantsReport" static,
     "Mutant" retest_all_mutant,
     "Mutant" dynamic_mutant,
     "Mutant" static_mutant

WHERE c.id = retest_all.commit_id
  AND c.id = dynamic.commit_id
  AND c.id = static.commit_id

  AND retest_all_mutant.report_id = retest_all.id
  AND dynamic_mutant.report_id = dynamic.id
  AND static_mutant.report_id = static.id

  AND retest_all.name = 'mutants'
  AND dynamic.name = 'mutants dynamic'
  AND static.name = 'mutants static'

  AND retest_all_mutant.descr = dynamic_mutant.descr
  AND retest_all_mutant.descr = static_mutant.descr

  AND retest_all_mutant.test_log is not null

  AND retest_all_mutant.test_result != 'TIMEOUT'
  AND dynamic_mutant.test_result != 'TIMEOUT'
  AND static_mutant.test_result != 'TIMEOUT';