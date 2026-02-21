locals {
  has_ssm_access             = length(var.ssm_parameter_arns) > 0
  has_secrets_manager_access = length(var.secrets_manager_arns) > 0
  has_kms_access             = length(var.kms_key_arns) > 0
  has_runtime_secret_access  = local.has_ssm_access || local.has_secrets_manager_access || local.has_kms_access
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
  dynamic "statement" {
    for_each = local.has_ssm_access ? [1] : []

    content {
      sid       = "ReadSsmParameters"
      actions   = ["ssm:GetParameter", "ssm:GetParameters", "ssm:GetParametersByPath"]
      resources = var.ssm_parameter_arns
    }
  }

  dynamic "statement" {
    for_each = local.has_secrets_manager_access ? [1] : []

    content {
      sid       = "ReadSecretsManager"
      actions   = ["secretsmanager:GetSecretValue", "secretsmanager:DescribeSecret"]
      resources = var.secrets_manager_arns
    }
  }

  dynamic "statement" {
    for_each = local.has_kms_access ? [1] : []

    content {
      sid       = "DecryptViaKms"
      actions   = ["kms:Decrypt"]
      resources = var.kms_key_arns
    }
  }
}

resource "aws_iam_policy" "runtime_access" {
  count = local.has_runtime_secret_access ? 1 : 0

  name   = "${var.name_prefix}-runtime-access"
  policy = data.aws_iam_policy_document.runtime_access.json
}

resource "aws_iam_role" "api_task" {
  name               = "${var.name_prefix}-api-task-role"
  assume_role_policy = data.aws_iam_policy_document.ecs_task_assume_role.json
}

resource "aws_iam_role_policy_attachment" "api_runtime_access" {
  count = local.has_runtime_secret_access ? 1 : 0

  role       = aws_iam_role.api_task.name
  policy_arn = aws_iam_policy.runtime_access[0].arn
}

resource "aws_iam_role" "worker_task" {
  name               = "${var.name_prefix}-worker-task-role"
  assume_role_policy = data.aws_iam_policy_document.ecs_task_assume_role.json
}

resource "aws_iam_role_policy_attachment" "worker_runtime_access" {
  count = local.has_runtime_secret_access ? 1 : 0

  role       = aws_iam_role.worker_task.name
  policy_arn = aws_iam_policy.runtime_access[0].arn
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
