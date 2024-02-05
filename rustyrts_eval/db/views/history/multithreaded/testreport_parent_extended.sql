-- this view just joins the TestReports of retest-all, dynamic and static on every parent commit
--CREATE VIEW testreport_parent_extended AS
SELECT c.id                             as commit,
       c.commit_str,
       c.repo_id,

       retest_all_report.id             as retest_all_id,
       retest_all_report.log            as retest_all_log,
       retest_all_report.duration       as retest_all_duration,
       retest_all_report.build_duration as retest_all_build_duration,

       dynamic_report.id                as dynamic_id,
       dynamic_report.log               as dynamic_log,
       dynamic_report.duration          as dynamic_duration,
       dynamic_report.build_duration    as dynamic_build_duration,

       static_report.id                 as static_id,
       static_report.log                as static_log,
       static_report.duration           as static_duration,
       static_report.build_duration     as static_build_duration


FROM "Commit" c,
     "TestReport" retest_all_report,
     "TestReport" dynamic_report,
     "TestReport" static_report

WHERE c.id = retest_all_report.commit_id
  AND c.id = dynamic_report.commit_id
  AND c.id = static_report.commit_id

  AND retest_all_report.name = 'cargo test - parent'
  AND dynamic_report.name = 'cargo rustyrts dynamic - parent'
  AND static_report.name = 'cargo rustyrts static - parent'

  AND retest_all_report.has_errored = false
  AND dynamic_report.has_errored = false
  AND static_report.has_errored = false
--;