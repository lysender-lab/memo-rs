variable "aws_region" {
  description = "AWS region where resources are created"
  type        = string
  default     = "ap-southeast-1"
}

variable "s3_bucket_name" {
  description = "S3 bucket name for storing memo-rs files"
  type        = string
  default     = "bucket-name"
}

variable "allowed_origins" {
  description = "Allowed browser origins for S3 CORS requests"
  type        = list(string)
  default     = ["http://localhost:3000", "https://example.com"]

  validation {
    condition     = length(var.allowed_origins) > 0
    error_message = "allowed_origins must contain at least one origin."
  }
}

variable "tags" {
  description = "Tags to apply to created resources"
  type        = map(string)
  default = {
    ManagedBy = "Terraform"
  }
}

variable "iam_role_name" {
  description = "IAM role name used by the memo-rs app"
  type        = string
  default     = "memo-rs-role-dev"
}

variable "iam_username" {
  description = "IAM username for programmatic access"
  type        = string
  default     = "memo-rs-user-dev"
}

variable "create_iam_user" {
  description = "Whether to create an IAM user for programmatic access"
  type        = bool
  default     = true
}

variable "create_iam_access_key" {
  description = "Whether to create a long-lived IAM access key for the IAM user"
  type        = bool
  default     = true

  validation {
    condition     = var.create_iam_access_key ? var.create_iam_user : true
    error_message = "create_iam_access_key can only be true when create_iam_user is true."
  }
}
