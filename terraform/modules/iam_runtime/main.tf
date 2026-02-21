locals {
  ssm_resources = length(var.ssm_parameter_arns) > 0 ? var.ssm_parameter_arns : ["*"]
  sm_resources  = length(var.secrets_manager_arns) > 0 ? var.secrets_manager_arns : ["*"]
}

data "aws_iam_policy_document" "ecs_task_assume_role" {
  statement {
    actions = ["sts:AssumeRole"]

    principals {
      type        = "Service"
      identifiers = ["ecs-tasks.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "ecs_task_execution" {
  name               = "${var.name_prefix}-ecs-exec-role"
  assume_role_policy = data.aws_iam_policy_document.ecs_task_assume_role.json
}

resource "aws_iam_role_policy_attachment" "ecs_task_execution_managed" {
  role       = aws_iam_role.ecs_task_execution.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy"
}

data "aws_iam_policy_document" "runtime_access" {
  statement {
    sid       = "ReadSsmParameters"
    actions   = ["ssm:GetParameter", "ssm:GetParameters", "ssm:GetParametersByPath"]
    resources = local.ssm_resources
  }

  statement {
    sid       = "ReadSecretsManager"
    actions   = ["secretsmanager:GetSecretValue", "secretsmanager:DescribeSecret"]
    resources = local.sm_resources
  }

  statement {
    sid       = "DecryptViaKms"
    actions   = ["kms:Decrypt"]
    resources = ["*"]
  }
}

resource "aws_iam_policy" "runtime_access" {
  name   = "${var.name_prefix}-runtime-access"
  policy = data.aws_iam_policy_document.runtime_access.json
}

resource "aws_iam_role" "api_task" {
  name               = "${var.name_prefix}-api-task-role"
  assume_role_policy = data.aws_iam_policy_document.ecs_task_assume_role.json
}

resource "aws_iam_role_policy_attachment" "api_runtime_access" {
  role       = aws_iam_role.api_task.name
  policy_arn = aws_iam_policy.runtime_access.arn
}

resource "aws_iam_role" "worker_task" {
  name               = "${var.name_prefix}-worker-task-role"
  assume_role_policy = data.aws_iam_policy_document.ecs_task_assume_role.json
}

resource "aws_iam_role_policy_attachment" "worker_runtime_access" {
  role       = aws_iam_role.worker_task.name
  policy_arn = aws_iam_policy.runtime_access.arn
}

data "aws_iam_policy_document" "ec2_assume_role" {
  statement {
    actions = ["sts:AssumeRole"]

    principals {
      type        = "Service"
      identifiers = ["ec2.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "enclave_host" {
  name               = "${var.name_prefix}-enclave-host-role"
  assume_role_policy = data.aws_iam_policy_document.ec2_assume_role.json
}

resource "aws_iam_role_policy_attachment" "enclave_ssm" {
  role       = aws_iam_role.enclave_host.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore"
}

resource "aws_iam_role_policy_attachment" "enclave_cloudwatch_agent" {
  role       = aws_iam_role.enclave_host.name
  policy_arn = "arn:aws:iam::aws:policy/CloudWatchAgentServerPolicy"
}

resource "aws_iam_instance_profile" "enclave_host" {
  name = "${var.name_prefix}-enclave-host-profile"
  role = aws_iam_role.enclave_host.name
}
